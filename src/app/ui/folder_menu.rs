//! Menu contestuale di una cartella (usato da treemap e Top 10).

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::system::finder::open_in_file_manager_label;
use crate::system::protection::is_deletion_forbidden;

/// Voci del menu contestuale di una cartella.
pub(in crate::app) fn folder_menu(
    ui: &mut egui::Ui,
    path: &Path,
    can_modify: bool,
    open_req: &mut Option<PathBuf>,
    del_req: &mut Option<(PathBuf, bool)>,
) {
    ui.label(path.display().to_string());
    ui.separator();
    if ui.button(open_in_file_manager_label()).clicked() {
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
