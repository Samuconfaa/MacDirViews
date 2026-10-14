<p align="center">
  <a href="README.md">English</a> &nbsp;|&nbsp;
  <strong>Italiano</strong> &nbsp;|&nbsp;
  <a href="README_FR.md">Français</a> &nbsp;|&nbsp;
  <a href="README_ES.md">Español</a> &nbsp;|&nbsp;
  <a href="README_RU.md">Русский</a>
</p>

***

# MacDirViews

Un visualizzatore di spazio su disco e strumento di pulizia moderno, veloce ed efficiente per macOS.

MacDirViews offre una rappresentazione grafica immediata e interattiva dello spazio occupato sul disco tramite treemap rettangolari (squarified treemap). Consente di individuare all'istante file e cartelle che saturano lo storage su dischi interni, memorie esterne o directory specifiche, rendendo l'analisi e la pulizia del Mac rapide, chiare e sicure.

---

## Perché scegliere MacDirViews?

- **Estremamente veloce e non bloccante**: Sviluppato in Rust con scansione multi-thread nativa. Analizza milioni di file in pochi secondi mantenendo l'interfaccia grafica costantemente fluida e reattiva a 60 FPS.
- **Precisione di calcolo per APFS**: Misura l'effettiva occupazione fisica su disco invece della sola dimensione nominale. Gestisce correttamente i file sparsi (sparse files) ed evita il doppio conteggio di cloni copy-on-write e hard link APFS.
- **Leggero ed efficiente**: Basato su rendering nativo accelerato da GPU. Consumo minimo di memoria RAM, senza framework web pesanti né overhead di runtime come Electron.
- **Sicurezza integrata**: Include controlli di protezione che impediscono la cancellazione accidentale di directory e file di sistema critici (`/`, `/System`, `/usr`, `/bin`, cartelle radice utente). Si integra direttamente con il Cestino di macOS per consentire il ripristino sicuro.

---

## Caratteristiche Principali

- **Treemap Squarified Interattiva**: Visualizza la dimensione di cartelle e file attraverso blocchi geometrici proporzionali. Un clic per visualizzare dettagli e percentuali; doppio clic per navigare ed entrare in qualsiasi sottocartella.
- **Classifiche Top 10 Cartelle e Top 20 File**: Elenchi dedicati che evidenziano le cartelle più pesanti nella schermata corrente e i singoli file più grandi dell'intera scansione.
- **Filtri e Ricerca in Tempo Reale**:
  - Ricerca testuale immediata per individuare elementi per nome.
  - Slider logaritmico della soglia minima per eliminare il rumore visivo delle cartelle di dimensioni trascurabili.
- **Supporto ai Volumi Esterni**: Selezione rapida di dischi esterni, chiavette USB e partizioni secondarie montate in `/Volumes`.
- **Drag & Drop**: Trascina qualsiasi cartella da una finestra del Finder direttamente nell'applicazione per avviare una scansione istantanea.
- **Integrazione con Finder e Cestino**: Clic destro su qualsiasi elemento per aprirlo nel Finder, spostarlo nel Cestino di macOS o procedere all'eliminazione definitiva con finestra di conferma esplicita.

---

## Permessi macOS (Accesso Completo al Disco)

macOS protegge i dati personali dell'utente (come Mail, Messaggi, cronologia Safari e librerie Foto) tramite il framework di sicurezza TCC (Transparency, Consent, and Control).

Per consentire a MacDirViews di analizzare l'intero disco di avvio senza errori di permessi:

1. Apri **Impostazioni di Sistema** → **Privacy e Sicurezza** → **Accesso completo al disco**.
2. Fai clic su **+** e aggiungi **MacDirViews** (o il binario compilato).
3. Riavvia l'applicazione.

*(È inoltre disponibile il pulsante **Full Disk Access** nella barra superiore dell'app per aprire direttamente la schermata delle impostazioni).*

---

## Installazione e Compilazione

### Requisiti
- macOS 11.0 (Big Sur) o versioni successive
- Apple Silicon (M1/M2/M3/M4) o Intel x86_64
- Toolchain Rust (`cargo`) e Xcode Command Line Tools

### Esecuzione da Codice Sorgente

```bash
cargo run --release
```

### Creazione del Bundle Applicativo (.app)

1. Installa lo strumento di packaging:
   ```bash
   cargo install cargo-bundle
   ```

2. Genera il bundle `.app`:
   ```bash
   cargo bundle --release
   ```

   Il file applicativo sarà disponibile in:
   ```
   target/release/bundle/osx/MacDirViews.app
   ```

3. Sposta l'applicazione nella cartella `/Applications`:
   ```bash
   cp -R target/release/bundle/osx/MacDirViews.app /Applications/
   ```

> [!NOTE]
> Se al primo avvio macOS Gatekeeper blocca l'applicazione compilata localmente non firmata, rimuovi il flag di quarantena dal terminale:
> ```bash
> xattr -cr /Applications/MacDirViews.app
> ```

---

## Licenza

Distribuito secondo i termini specificati nel repository.
