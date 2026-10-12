//! DiskScope - clone moderno di WinDirStat per macOS.
//!
//! Architettura:
//! - La scansione gira in un thread separato (`std::thread::spawn`), cosi' la UI non si blocca.
//! - Stato condiviso: `Arc<Mutex<bool>>` (scansione in corso) e
//!   `Arc<Mutex<HashMap<PathBuf, u64>>>` (dimensione cumulativa di ogni cartella).
//! - La UI NON tiene mai il lock durante il disegno: `refresh()` copia i dati che servono
//!   in strutture locali e rilascia subito il mutex.
//! - La treemap e' disegnata solo con il painter nativo di egui (rettangoli e testo).

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt; // per `blocks()` (spazio realmente occupato su disco)
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke};
use jwalk::WalkDir;

/// Numero massimo di blocchi disegnati nella treemap (il resto viene raggruppato in "altri").
const MAX_BLOCKS: usize = 50;

// ---------------------------------------------------------------------------
// Utilita'
// ---------------------------------------------------------------------------

/// Restituisce `true` per le cartelle di sistema macOS che vanno saltate
/// (protette da SIP/TCC, virtuali, oppure che causerebbero loop o permessi negati).
fn is_protected_directory(path: &Path) -> bool {
    // Con i permessi admin si scandisce tutto, tranne le cartelle che falserebbero
    // il conteggio o sono virtuali. Il confronto e' esatto: se la cartella viene
    // esclusa, non si scende nemmeno nei suoi figli.
    const SKIPPED: &[&str] = &[
        "/System/Volumes", // firmlink: il volume Data e' gia' raggiungibile da "/" (doppio conteggio)
        "/Volumes",        // altri dischi montati + link al disco di avvio (loop)
        "/dev",            // file di dispositivo
        "/Network",        // montaggi di rete
        "/net",            // autofs: puo' bloccarsi
        "/home",           // autofs: puo' bloccarsi
        "/.vol",           // accesso per ID, duplica tutto
        "/cores",
    ];
    SKIPPED.iter().any(|p| path == Path::new(p))
}

/// `true` se il processo gira come root (uid 0).
fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

/// Crea un comando che gira nella sessione grafica dell'utente loggato.
/// Come root si passa da `launchctl asuser <uid>`, altrimenti il comando e' diretto.
fn user_session_command(is_admin: bool, program: &str) -> Command {
    if is_admin {
        let uid = std::env::var("SUDO_UID")
            .ok()
            .filter(|u| !u.is_empty())
            .or_else(|| {
                Command::new("stat")
                    .args(["-f", "%u", "/dev/console"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_else(|| "501".to_string());
        let mut c = Command::new("launchctl");
        c.arg("asuser").arg(uid).arg(program);
        c
    } else {
        Command::new(program)
    }
}

/// Cartelle che non si devono mai poter eliminare dall'app: la radice, le cartelle
/// di primo livello, le home degli utenti, gli antenati della propria Home e le
/// cartelle di sistema.
fn is_deletion_forbidden(path: &Path) -> bool {
    match path.parent() {
        None => return true,
        Some(p) if p == Path::new("/") || p == Path::new("/Users") => return true,
        _ => {}
    }
    if let Some(home) = dirs_next::home_dir() {
        if home.starts_with(path) {
            return true; // la Home stessa o una cartella che la contiene
        }
    }
    const SYSTEM: &[&str] = &[
        "/System", "/usr", "/bin", "/sbin", "/etc", "/dev", "/cores", "/Volumes", "/private",
    ];
    SYSTEM.iter().any(|p| path.starts_with(p))
}

/// Sposta una cartella nel Cestino tramite il Finder.
fn move_to_trash(path: &Path, is_admin: bool) -> Result<(), String> {
    let p = path.to_string_lossy();
    if p.contains('"') || p.contains('\\') {
        return Err("il percorso contiene caratteri non supportati".into());
    }
    let mut cmd = user_session_command(is_admin, "osascript");
    cmd.arg("-e")
        .arg(format!(r#"tell application "Finder" to delete POSIX file "{}""#, p));
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Voci del menu contestuale di una cartella (usato da treemap e Top 10).
fn folder_menu(
    ui: &mut egui::Ui,
    path: &Path,
    can_modify: bool,
    open_req: &mut Option<PathBuf>,
    del_req: &mut Option<(PathBuf, bool)>,
) {
    ui.label(path.display().to_string());
    ui.separator();
    if ui.button("Apri nel Finder").clicked() {
        *open_req = Some(path.to_path_buf());
        ui.close_menu();
    }
    let allowed = can_modify && !is_deletion_forbidden(path);
    if ui
        .add_enabled(allowed, egui::Button::new("Sposta nel Cestino..."))
        .clicked()
    {
        *del_req = Some((path.to_path_buf(), false));
        ui.close_menu();
    }
    if ui
        .add_enabled(allowed, egui::Button::new("Elimina definitivamente..."))
        .clicked()
    {
        *del_req = Some((path.to_path_buf(), true));
        ui.close_menu();
    }
    if !allowed {
        let why = if can_modify {
            "Cartella protetta: non eliminabile da qui."
        } else {
            "Eliminazione disponibile a scansione finita."
        };
        ui.label(why);
    }
}

/// Converte byte in una stringa leggibile in MB o GB.
fn fmt_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else {
        format!("{:.2} MB", b / MB)
    }
}

/// Colore (HSV -> RGB) distinto per ogni indice di blocco.
fn color_for(index: usize) -> Color32 {
    let h = (index as f32 * 0.137).fract();
    let (s, v) = (0.45_f32, 0.85_f32);
    let channel = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).min(1.0).max(0.0)
    };
    Color32::from_rgb(
        (channel(5.0) * 255.0) as u8,
        (channel(3.0) * 255.0) as u8,
        (channel(1.0) * 255.0) as u8,
    )
}

/// Dalle dimensioni "dirette" (solo i file contenuti in ogni cartella) calcola
/// le dimensioni cumulative, propagando ogni valore a tutte le cartelle antenate fino a `root`.
fn rollup(direct: &HashMap<PathBuf, u64>, root: &Path) -> HashMap<PathBuf, u64> {
    let mut out: HashMap<PathBuf, u64> = HashMap::with_capacity(direct.len());
    for (dir, &size) in direct {
        for ancestor in dir.ancestors() {
            *out.entry(ancestor.to_path_buf()).or_insert(0) += size;
            if ancestor == root {
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Scansione (thread separato)
// ---------------------------------------------------------------------------

/// Cammina l'albero a partire da `root` e riempie `sizes`.
/// Gira nel thread di scansione; aggiorna periodicamente la mappa condivisa.
fn scan_directory(
    root: PathBuf,
    scanning: Arc<Mutex<bool>>,
    sizes: Arc<Mutex<HashMap<PathBuf, u64>>>,
    files_counter: Arc<AtomicU64>,
    ctx: egui::Context,
) {
    // Dimensioni dirette per cartella, tenute in locale per non contendere il mutex.
    let mut direct: HashMap<PathBuf, u64> = HashMap::new();
    direct.insert(root.clone(), 0);

    let walker = WalkDir::new(&root)
        .skip_hidden(false)
        .follow_links(false)
        .process_read_dir(|_depth, _path, _state, children| {
            // Salta le cartelle protette prima ancora di entrarci.
            children.retain(|entry| match entry {
                Ok(e) => !(e.file_type().is_dir() && is_protected_directory(&e.path())),
                Err(_) => true, // gli errori (permessi negati) vengono ignorati dopo
            });
        });

    let mut last_publish = Instant::now();
    let mut seen: u64 = 0;

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // permesso negato o file sparito: si ignora
        };
        if !entry.file_type().is_file() {
            continue; // niente symlink/cartelle: contiamo solo i file
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Spazio realmente allocato su disco (blocchi da 512 byte).
        let size = meta.blocks() * 512;

        let path = entry.path();
        if let Some(parent) = path.parent() {
            if let Some(v) = direct.get_mut(parent) {
                *v += size;
            } else {
                direct.insert(parent.to_path_buf(), size);
            }
        }

        seen += 1;
        if seen % 1000 == 0 {
            files_counter.store(seen, Ordering::Relaxed);
            // Pubblica un'istantanea circa ogni secondo, cosi' la UI mostra dati parziali.
            if last_publish.elapsed() >= Duration::from_secs(1) {
                let snapshot = rollup(&direct, &root);
                *sizes.lock().unwrap() = snapshot;
                last_publish = Instant::now();
                ctx.request_repaint();
            }
        }
    }

    // Risultato finale.
    files_counter.store(seen, Ordering::Relaxed);
    let snapshot = rollup(&direct, &root);
    *sizes.lock().unwrap() = snapshot;
    *scanning.lock().unwrap() = false;
    ctx.request_repaint();
}

// ---------------------------------------------------------------------------
// Applicazione
// ---------------------------------------------------------------------------

/// Un blocco della treemap.
struct Block {
    path: PathBuf,
    label: String,
    size: u64,
    /// `true` se e' una sottocartella navigabile (non i blocchi speciali "file" / "altri").
    is_dir: bool,
}

struct DiskApp {
    path_input: String,
    scanning: Arc<Mutex<bool>>,
    sizes: Arc<Mutex<HashMap<PathBuf, u64>>>,
    files_counter: Arc<AtomicU64>,

    scan_root: PathBuf,
    view_dir: PathBuf,

    // Dati derivati, copiati fuori dal mutex da `refresh()`.
    top10: Vec<(PathBuf, u64)>,
    blocks: Vec<Block>,
    total_size: u64,

    selected: Option<PathBuf>,
    status: Option<String>,
    was_scanning: bool,
    last_refresh: Instant,

    // Cronologia di navigazione (Indietro / Avanti).
    back_stack: Vec<PathBuf>,
    fwd_stack: Vec<PathBuf>,

    // Permessi amministratore.
    is_admin: bool,
    admin_msg: Arc<Mutex<Option<String>>>,

    // Eliminazione: bersaglio del menu contestuale, richiesta in attesa di conferma
    // (percorso, definitiva?) ed esito del thread di eliminazione.
    ctx_target: Option<PathBuf>,
    pending_delete: Option<(PathBuf, bool)>,
    delete_result: Arc<Mutex<Option<(PathBuf, Result<(), String>)>>>,
}

impl Default for DiskApp {
    fn default() -> Self {
        Self {
            path_input: String::new(),
            scanning: Arc::new(Mutex::new(false)),
            sizes: Arc::new(Mutex::new(HashMap::new())),
            files_counter: Arc::new(AtomicU64::new(0)),
            scan_root: PathBuf::new(),
            view_dir: PathBuf::new(),
            top10: Vec::new(),
            blocks: Vec::new(),
            total_size: 0,
            selected: None,
            status: None,
            was_scanning: false,
            last_refresh: Instant::now(),
            back_stack: Vec::new(),
            fwd_stack: Vec::new(),
            is_admin: is_root(),
            admin_msg: Arc::new(Mutex::new(None)),
            ctx_target: None,
            pending_delete: None,
            delete_result: Arc::new(Mutex::new(None)),
        }
    }
}

impl DiskApp {
    /// Avvia la scansione in un thread separato.
    fn start_scan(&mut self, ctx: &egui::Context) {
        let input = self.path_input.trim().to_string();

        // Percorso vuoto -> cartella Home; supporto base per "~".
        let root = if input.is_empty() {
            match dirs_next::home_dir() {
                Some(home) => home,
                None => {
                    self.status = Some("Impossibile determinare la cartella Home.".into());
                    return;
                }
            }
        } else if input == "~" || input.starts_with("~/") {
            match dirs_next::home_dir() {
                Some(home) => home.join(input.trim_start_matches('~').trim_start_matches('/')),
                None => {
                    self.status = Some("Impossibile determinare la cartella Home.".into());
                    return;
                }
            }
        } else {
            PathBuf::from(&input)
        };

        if !root.is_dir() {
            self.status = Some(format!("Non e' una cartella valida: {}", root.display()));
            return;
        }

        // Reset dello stato.
        self.status = None;
        self.selected = None;
        self.top10.clear();
        self.blocks.clear();
        self.total_size = 0;
        self.scan_root = root.clone();
        self.view_dir = root.clone();
        self.back_stack.clear();
        self.fwd_stack.clear();
        self.files_counter.store(0, Ordering::Relaxed);
        self.sizes.lock().unwrap().clear();
        *self.scanning.lock().unwrap() = true;
        self.last_refresh = Instant::now();

        // Cloni per il thread.
        let scanning = Arc::clone(&self.scanning);
        let sizes = Arc::clone(&self.sizes);
        let counter = Arc::clone(&self.files_counter);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            scan_directory(root, scanning, sizes, counter, ctx);
        });
    }

    /// Ricalcola top-10 e blocchi della treemap dalla mappa condivisa.
    /// Il mutex viene rilasciato esplicitamente PRIMA di toccare qualsiasi altro campo.
    fn refresh(&mut self) {
        let view = self.view_dir.clone();
        let root = self.scan_root.clone();

        let guard = self.sizes.lock().unwrap();

        // Top 10 delle cartelle contenute (a qualsiasi profondita') nella cartella
        // in cui si sta navigando, esclusa la cartella stessa.
        let mut all: Vec<(&PathBuf, u64)> = guard
            .iter()
            .filter(|(p, _)| **p != view && p.starts_with(&view))
            .map(|(p, s)| (p, *s))
            .collect();
        if all.len() > 10 {
            all.select_nth_unstable_by(10, |a, b| b.1.cmp(&a.1));
            all.truncate(10);
        }
        all.sort_by(|a, b| b.1.cmp(&a.1));
        let top10: Vec<(PathBuf, u64)> = all.into_iter().map(|(p, s)| (p.clone(), s)).collect();

        // Figli diretti della cartella visualizzata.
        let view_size = guard.get(&view).copied().unwrap_or(0);
        let root_size = guard.get(&root).copied().unwrap_or(0);
        let mut children: Vec<(PathBuf, u64)> = guard
            .iter()
            .filter(|(p, _)| **p != view && p.parent() == Some(view.as_path()))
            .map(|(p, s)| (p.clone(), *s))
            .collect();

        // Lock rilasciato qui: da ora si lavora solo su copie locali.
        drop(guard);

        children.sort_by(|a, b| b.1.cmp(&a.1));
        let children_sum: u64 = children.iter().map(|c| c.1).sum();
        let own_files = view_size.saturating_sub(children_sum);

        let mut blocks: Vec<Block> = Vec::new();
        let mut rest_size: u64 = 0;
        let mut rest_count: usize = 0;
        for (i, (path, size)) in children.into_iter().enumerate() {
            if size == 0 {
                continue;
            }
            if i < MAX_BLOCKS {
                let label = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                blocks.push(Block { path, label, size, is_dir: true });
            } else {
                rest_size += size;
                rest_count += 1;
            }
        }
        if rest_size > 0 {
            blocks.push(Block {
                path: view.clone(),
                label: format!("(altre {} cartelle)", rest_count),
                size: rest_size,
                is_dir: false,
            });
        }
        if own_files > 0 {
            blocks.push(Block {
                path: view.clone(),
                label: "(file in questa cartella)".to_string(),
                size: own_files,
                is_dir: false,
            });
        }
        blocks.sort_by(|a, b| b.size.cmp(&a.size));

        self.top10 = top10;
        self.blocks = blocks;
        self.total_size = root_size;
        self.last_refresh = Instant::now();
    }

    /// Naviga in una cartella registrando la posizione corrente nella cronologia.
    fn navigate_to(&mut self, dir: PathBuf) {
        if dir == self.view_dir {
            return;
        }
        let current = self.view_dir.clone();
        self.back_stack.push(current);
        self.fwd_stack.clear();
        self.view_dir = dir;
        self.selected = None;
        self.refresh();
    }

    /// Torna alla cartella visitata prima.
    fn go_back(&mut self) {
        if let Some(prev) = self.back_stack.pop() {
            let current = std::mem::replace(&mut self.view_dir, prev);
            self.fwd_stack.push(current);
            self.selected = None;
            self.refresh();
        }
    }

    /// Va avanti dopo un "Indietro".
    fn go_forward(&mut self) {
        if let Some(next) = self.fwd_stack.pop() {
            let current = std::mem::replace(&mut self.view_dir, next);
            self.back_stack.push(current);
            self.selected = None;
            self.refresh();
        }
    }

    /// Rilancia l'app con permessi di amministratore (richiede la password di macOS)
    /// e chiude l'istanza corrente solo se l'avvio riesce.
    fn relaunch_as_admin(&mut self, ctx: &egui::Context) {
        let exe = match std::env::current_exe() {
            Ok(e) => e.to_string_lossy().to_string(),
            Err(e) => {
                self.status = Some(format!("Percorso dell'eseguibile non disponibile: {e}"));
                return;
            }
        };
        if exe.contains('"') || exe.contains('\\') || exe.contains('\'') {
            self.status = Some(
                "Il percorso dell'app contiene caratteri non supportati: usa `sudo` da Terminale."
                    .into(),
            );
            return;
        }
        let script = format!(
            r#"do shell script "\"{}\" >/dev/null 2>&1 &" with administrator privileges"#,
            exe
        );
        let msg = Arc::clone(&self.admin_msg);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            match Command::new("osascript").arg("-e").arg(script).output() {
                Ok(out) if out.status.success() => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    *msg.lock().unwrap() =
                        Some(format!("Avvio come amministratore annullato o fallito: {err}"));
                    ctx.request_repaint();
                }
                Err(e) => {
                    *msg.lock().unwrap() = Some(format!("Impossibile eseguire osascript: {e}"));
                    ctx.request_repaint();
                }
            }
        });
    }

    /// Avvia l'eliminazione in un thread (Cestino oppure definitiva).
    fn start_delete(&mut self, ctx: &egui::Context, path: PathBuf, permanent: bool) {
        if is_deletion_forbidden(&path) {
            self.status = Some(format!("Cartella protetta, non eliminabile: {}", path.display()));
            return;
        }
        self.status = Some(format!("Eliminazione in corso: {}", path.display()));
        let result = Arc::clone(&self.delete_result);
        let is_admin = self.is_admin;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = if permanent {
                std::fs::remove_dir_all(&path).map_err(|e| e.to_string())
            } else {
                move_to_trash(&path, is_admin)
            };
            *result.lock().unwrap() = Some((path, res));
            ctx.request_repaint();
        });
    }

    /// Dopo un'eliminazione riuscita aggiorna dimensioni, navigazione e treemap
    /// senza dover rifare la scansione.
    fn apply_deletion(&mut self, path: &Path) {
        {
            let mut map = self.sizes.lock().unwrap();
            let removed = map.get(path).copied().unwrap_or(0);
            map.retain(|p, _| !p.starts_with(path));
            for anc in path.ancestors().skip(1) {
                if let Some(v) = map.get_mut(anc) {
                    *v = v.saturating_sub(removed);
                }
                if anc == self.scan_root.as_path() {
                    break;
                }
            }
        }
        self.back_stack.retain(|p| !p.starts_with(path));
        self.fwd_stack.retain(|p| !p.starts_with(path));
        if self.selected.as_ref().map_or(false, |s| s.starts_with(path)) {
            self.selected = None;
        }
        if self.view_dir.starts_with(path) {
            if let Some(parent) = path.parent() {
                self.view_dir = parent.to_path_buf();
            }
        }
        self.refresh();
    }

    /// Apre il percorso selezionato nel Finder di macOS.
    fn open_in_finder(&mut self, path: &Path) {
        // Come root, `open` da solo non raggiunge la sessione grafica dell'utente.
        let mut cmd = user_session_command(self.is_admin, "open");
        cmd.arg("-R").arg(path);

        // `output()` attende la fine (e' immediato) e permette di mostrare l'errore reale.
        match cmd.output() {
            Ok(out) if out.status.success() => {
                self.status = None;
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.status = Some(format!("Apertura nel Finder fallita: {err}"));
            }
            Err(e) => {
                self.status = Some(format!("Impossibile aprire il Finder: {e}"));
            }
        }
    }

    /// Pannello centrale: breadcrumb, treemap, informazioni e pulsante Finder.
    fn central_ui(&mut self, ui: &mut egui::Ui, scanning: bool) {
        // --- Barra di navigazione ---
        let mut go_up = false;
        let mut go_back = false;
        let mut go_fwd = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.back_stack.is_empty(), egui::Button::new("< Indietro"))
                .clicked()
            {
                go_back = true;
            }
            if ui
                .add_enabled(!self.fwd_stack.is_empty(), egui::Button::new("Avanti >"))
                .clicked()
            {
                go_fwd = true;
            }
            let can_up = self.view_dir != self.scan_root
                && self.view_dir.parent().is_some()
                && !self.scan_root.as_os_str().is_empty();
            if ui.add_enabled(can_up, egui::Button::new("Su")).clicked() {
                go_up = true;
            }
            ui.label(self.view_dir.display().to_string());
        });
        // Tasto "indietro" del mouse (se presente).
        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
            go_back = true;
        }
        if go_back {
            self.go_back();
        } else if go_fwd {
            self.go_forward();
        } else if go_up {
            if let Some(parent) = self.view_dir.parent().map(|p| p.to_path_buf()) {
                self.navigate_to(parent);
            }
        }

        if scanning {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "Scansione in corso... {} file analizzati",
                    self.files_counter.load(Ordering::Relaxed)
                ));
            });
        } else if self.total_size > 0 {
            ui.label(format!("Totale scansionato: {}", fmt_size(self.total_size)));
        }
        if let Some(msg) = &self.status {
            ui.colored_label(Color32::LIGHT_RED, msg);
        }

        ui.add_space(4.0);

        // --- Treemap orizzontale ---
        let avail = ui.available_size();
        let size = egui::vec2(avail.x.max(50.0), (avail.y - 80.0).max(100.0));
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click());
        let rect = response.rect;

        painter.rect_filled(rect, 0.0, Color32::from_gray(30));

        let total: u64 = self.blocks.iter().map(|b| b.size).sum();
        let pointer: Option<Pos2> = response.hover_pos();
        let mut hovered: Option<usize> = None;

        if total > 0 {
            let n = self.blocks.len();
            let mut x = rect.left();
            for (i, block) in self.blocks.iter().enumerate() {
                // Larghezza proporzionale alla dimensione; l'ultimo blocco chiude lo spazio.
                let w = if i + 1 == n {
                    (rect.right() - x).max(0.0)
                } else {
                    rect.width() * (block.size as f32 / total as f32)
                };
                let r = Rect::from_min_size(Pos2::new(x, rect.top()), egui::vec2(w, rect.height()));
                x += w;

                let is_hover = pointer.map_or(false, |p| r.contains(p));
                if is_hover {
                    hovered = Some(i);
                }
                let is_selected = self.selected.as_ref() == Some(&block.path)
                    && block.is_dir;

                let mut color = if block.is_dir { color_for(i) } else { Color32::from_gray(150) };
                if is_hover {
                    color = color.gamma_multiply(1.15);
                }
                let inner = r.shrink(1.0);
                painter.rect_filled(inner, 2.0, color);
                if is_selected {
                    painter.rect_stroke(inner, 2.0, Stroke::new(3.0_f32, Color32::WHITE));
                }

                // Etichetta solo se c'e' abbastanza spazio, ritagliata sul blocco.
                if w > 60.0 {
                    let clipped = painter.with_clip_rect(inner);
                    clipped.text(
                        inner.left_top() + egui::vec2(4.0, 4.0),
                        Align2::LEFT_TOP,
                        &block.label,
                        FontId::proportional(13.0),
                        Color32::BLACK,
                    );
                    clipped.text(
                        inner.left_top() + egui::vec2(4.0, 22.0),
                        Align2::LEFT_TOP,
                        fmt_size(block.size),
                        FontId::proportional(12.0),
                        Color32::from_black_alpha(200),
                    );
                }
            }
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                if scanning { "Scansione in corso..." } else { "Premi \"Start Scan\" per iniziare" },
                FontId::proportional(16.0),
                Color32::GRAY,
            );
        }

        // Tasto destro: seleziona il blocco sotto il mouse e lo usa per il menu contestuale.
        if response.secondary_clicked() {
            self.ctx_target = hovered.and_then(|i| {
                let b = &self.blocks[i];
                if b.is_dir {
                    Some(b.path.clone())
                } else {
                    None
                }
            });
            if let Some(p) = self.ctx_target.clone() {
                self.selected = Some(p);
            }
        }

        // Click = seleziona, doppio click = entra nella cartella.
        let mut new_view: Option<PathBuf> = None;
        if let Some(i) = hovered {
            let block = &self.blocks[i];
            if response.clicked() {
                self.selected = Some(block.path.clone());
            }
            if response.double_clicked() && block.is_dir {
                new_view = Some(block.path.clone());
            }
        }

        // --- Pannello informazioni (hover / selezione) ---
        ui.add_space(6.0);
        ui.separator();
        let mut open_path: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            // Senza selezione si apre la cartella che si sta visualizzando.
            let can_open = self.selected.is_some() || !self.view_dir.as_os_str().is_empty();
            if ui.add_enabled(can_open, egui::Button::new("Open in Finder")).clicked() {
                open_path = self.selected.clone().or_else(|| Some(self.view_dir.clone()));
            }
            match hovered {
                Some(i) => {
                    let b = &self.blocks[i];
                    let pct = if total > 0 { b.size as f64 * 100.0 / total as f64 } else { 0.0 };
                    ui.label(format!("{}  -  {} ({:.1}%)", b.path.display(), fmt_size(b.size), pct));
                }
                None => match &self.selected {
                    Some(p) => {
                        ui.label(format!("Selezionato: {}", p.display()));
                    }
                    None => {
                        ui.label("Clic = seleziona, doppio clic = entra nella cartella.");
                    }
                },
            }
        });

        // Menu contestuale (tasto destro) sulla treemap.
        let can_modify = !scanning;
        let mut del_req: Option<(PathBuf, bool)> = None;
        response.context_menu(|ui| match self.ctx_target.clone() {
            Some(p) => folder_menu(ui, &p, can_modify, &mut open_path, &mut del_req),
            None => {
                ui.label("Nessuna cartella qui.");
            }
        });
        if let Some(d) = del_req {
            self.pending_delete = Some(d);
        }

        // Tooltip che segue il mouse con percorso completo e dimensione.
        if let Some(i) = hovered {
            let b = &self.blocks[i];
            let tip = format!("{}\n{}", b.path.display(), fmt_size(b.size));
            response.on_hover_text(tip);
        }

        if let Some(p) = open_path {
            self.open_in_finder(&p);
        }
        if let Some(dir) = new_view {
            self.navigate_to(dir);
        }
    }
}

impl eframe::App for DiskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Lettura dello stato: il guard e' un temporaneo e viene rilasciato subito.
        let scanning = *self.scanning.lock().unwrap();

        // Eventuale errore dal thread di avvio come amministratore.
        let admin_err = self.admin_msg.lock().unwrap().take();
        if let Some(m) = admin_err {
            self.status = Some(m);
        }

        // Esito di un'eliminazione terminata nel thread.
        let delete_done = self.delete_result.lock().unwrap().take();
        if let Some((path, res)) = delete_done {
            match res {
                Ok(()) => {
                    self.apply_deletion(&path);
                    self.status = None;
                }
                Err(e) => self.status = Some(format!("Eliminazione fallita: {e}")),
            }
        }

        if scanning {
            // Durante la scansione la UI resta fluida: ridisegno periodico + aggiornamento dati.
            ctx.request_repaint_after(Duration::from_millis(100));
            if self.last_refresh.elapsed() >= Duration::from_millis(500) {
                self.refresh();
            }
        } else if self.was_scanning {
            // Scansione appena terminata: ultimo aggiornamento con i dati finali.
            self.refresh();
            ctx.request_repaint();
        }
        self.was_scanning = scanning;

        // --- Barra superiore ---
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Percorso:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("vuoto = cartella Home")
                        .desired_width(320.0),
                );
                if ui.add_enabled(!scanning, egui::Button::new("Start Scan")).clicked() {
                    self.start_scan(ctx);
                }
                if ui
                    .add_enabled(!scanning, egui::Button::new("Scansiona tutto il disco (/)"))
                    .clicked()
                {
                    self.path_input = "/".to_string();
                    self.start_scan(ctx);
                }
                ui.separator();
                if self.is_admin {
                    ui.colored_label(Color32::LIGHT_GREEN, "Admin: attivo");
                } else {
                    ui.colored_label(Color32::YELLOW, "Admin: non attivo");
                    if ui.add_enabled(!scanning, egui::Button::new("Riavvia come amministratore")).clicked() {
                        self.relaunch_as_admin(ctx);
                    }
                }
            });
            ui.add_space(6.0);
        });

        // --- Classifica delle 10 cartelle piu' grandi ---
        let mut clicked_path: Option<PathBuf> = None;
        let mut side_open: Option<PathBuf> = None;
        let mut side_del: Option<(PathBuf, bool)> = None;
        let can_modify = !scanning;
        egui::SidePanel::left("top10_panel")
            .min_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Top 10 cartelle");
                ui.label(format!("in: {}", self.view_dir.display()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.top10.is_empty() {
                        ui.label("Nessun dato.");
                    }
                    for (i, (path, size)) in self.top10.iter().enumerate() {
                        let text = format!("{}. {}\n{}", i + 1, fmt_size(*size), path.display());
                        let is_sel = self.selected.as_ref() == Some(path);
                        let resp = ui.selectable_label(is_sel, text);
                        if resp.clicked() {
                            clicked_path = Some(path.clone());
                        }
                        // Tasto destro anche sulle voci della Top 10.
                        resp.context_menu(|ui| {
                            folder_menu(ui, path, can_modify, &mut side_open, &mut side_del);
                        });
                        ui.add_space(2.0);
                    }
                });
            });
        if let Some(p) = clicked_path {
            self.selected = Some(p);
        }
        if let Some(p) = side_open {
            self.open_in_finder(&p);
        }
        if let Some(d) = side_del {
            self.pending_delete = Some(d);
        }

        // --- Treemap e dettagli ---
        egui::CentralPanel::default().show(ctx, |ui| {
            self.central_ui(ui, scanning);
        });

        // --- Finestra di conferma eliminazione ---
        if let Some((path, permanent)) = self.pending_delete.clone() {
            let size = self.sizes.lock().unwrap().get(&path).copied().unwrap_or(0);
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("Conferma eliminazione")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(path.display().to_string());
                    ui.label(format!("Dimensione: {}", fmt_size(size)));
                    ui.add_space(6.0);
                    if permanent {
                        ui.colored_label(
                            Color32::LIGHT_RED,
                            "ATTENZIONE: eliminazione definitiva, non si puo' annullare.",
                        );
                    } else {
                        ui.label("La cartella verra' spostata nel Cestino.");
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let label = if permanent { "Elimina definitivamente" } else { "Sposta nel Cestino" };
                        if ui.button(label).clicked() {
                            confirm = true;
                        }
                        if ui.button("Annulla").clicked() {
                            cancel = true;
                        }
                    });
                });
            if confirm {
                self.pending_delete = None;
                self.start_delete(ctx, path, permanent);
            } else if cancel {
                self.pending_delete = None;
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([700.0, 450.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DiskScope",
        options,
        Box::new(|_cc| Ok(Box::new(DiskApp::default()))),
    )
}