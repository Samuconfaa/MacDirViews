<p align="center">
  <a href="README.md">English</a> &nbsp;|&nbsp;
  <a href="README_IT.md">Italiano</a> &nbsp;|&nbsp;
  <a href="README_FR.md">Français</a> &nbsp;|&nbsp;
  <strong>Español</strong> &nbsp;|&nbsp;
  <a href="README_RU.md">Русский</a>
</p>

***

# DirTreeView

Un visualizador de espacio en disco y herramienta de limpieza moderno, rápido y eficiente para macOS.

DirTreeView proporciona una vista gráfica intuitiva e interactiva del uso de almacenamiento mediante treemaps rectangulares (squarified treemaps). Le permite identificar al instante qué archivos y carpetas están consumiendo espacio en unidades internas, discos externos o carpetas específicas, logrando que el análisis y la liberación de espacio sean rápidos, claros y seguros.

---

## ¿Por qué elegir DirTreeView?

- **Ultrarrápido y no bloqueante**: Desarrollado en Rust con motor de escaneo multihilo nativo. Analiza millones de archivos en cuestión de segundos manteniendo la interfaz gráfica fluida y con una respuesta constante a 60 FPS.
- **Cálculo preciso para APFS**: Mide la ocupación física real en el disco en lugar de basarse únicamente en el tamaño nominal. Gestiona correctamente archivos dispersos (sparse files) y evita duplicar el conteo de enlaces duros o clones copy-on-write de APFS.
- **Ligero y optimizado**: Construido con un motor gráfico nativo acelerado por GPU. Consume una cantidad mínima de memoria RAM, sin marcos web pesados ni el consumo excesivo de Electron.
- **Seguridad integrada**: Incluye protecciones que impiden la eliminación accidental de directorios y archivos críticos del sistema operativo (`/`, `/System`, `/usr`, `/bin`, carpetas raíz de usuario). Se integra directamente con la Papelera de macOS para permitir la recuperación segura.

---

## Características Principales

- **Treemap Squarified Interactivo**: Muestra el tamaño de directorios y archivos mediante bloques geométricos proporcionales. Un clic para inspeccionar detalles y porcentajes; doble clic para navegar dentro de cualquier subcarpeta.
- **Clasificaciones Top 10 Carpetas y Top 20 Archivos**: Secciones dedicadas que destacan las carpetas más pesadas en la vista actual y los archivos individuales más grandes de todo el escaneo.
- **Filtros y Búsqueda en Tiempo Real**:
  - Búsqueda de texto instantánea para localizar elementos por nombre.
  - Control deslizante logarítmico de tamaño mínimo para ocultar elementos de tamaño insignificante.
- **Compatibilidad con Volúmenes Externos**: Selector integrado para escanear rápidamente discos externos, memorias USB y particiones secundarias montadas en `/Volumes`.
- **Arrastrar y Soltar (Drag & Drop)**: Arrastre cualquier carpeta directamente desde Finder hacia la ventana para iniciar un escaneo inmediato.
- **Integración con Finder y Papelera**: Clic derecho sobre cualquier elemento para mostrarlo en Finder, moverlo a la Papelera de macOS o eliminarlo de forma permanente mediante una confirmación explícita.

---

## Instalación y Descarga

### Paquetes Precompilados (.dmg / .app)

**No es necesario compilar el código fuente.** Las imágenes de disco `.dmg` y los paquetes `.app` listos para usar se publican en cada versión:

1. Descargue la última versión de **DirTreeView** desde [GitHub Releases](https://github.com/Samuconfaa/DirTreeView/releases).
2. Abra el archivo `.dmg` y arrastre **DirTreeView** a la carpeta de **Aplicaciones** (`/Applications`).

> [!NOTE]
> Si macOS Gatekeeper bloquea la apertura de una versión no firmada descargada desde la web, haga clic derecho sobre la aplicación → **Abrir**, o ejecute el siguiente comando en la Terminal:
> ```bash
> xattr -cr /Applications/DirTreeView.app
> ```

### Compilación desde el Código Fuente (Opcional)

Si prefiere compilar manualmente el proyecto:

#### Requisitos
- macOS 11.0 (Big Sur) o posterior
- Apple Silicon (M1/M2/M3/M4) o Intel x86_64
- Toolchain de Rust (`cargo`) y Xcode Command Line Tools

```bash
# Ejecutar directamente
cargo run --release

# Generar el paquete .app
cargo install cargo-bundle
cargo bundle --release
```

---

## Permisos de macOS (Acceso Total al Disco)

macOS restringe el acceso a carpetas con datos personales del usuario (como Mail, Mensajes, historial de Safari y fototecas) mediante el sistema de seguridad TCC (Transparency, Consent, and Control).

Para permitir que DirTreeView analice todo el disco de arranque sin errores de permisos:

1. Abra **Ajustes del Sistema** → **Privacidad y seguridad** → **Acceso total al disco**.
2. Haga clic en **+** y añada **DirTreeView** (o el binario compilado).
3. Reinicie la aplicación.

*(También puede utilizar el botón **Full Disk Access** situado en la barra superior de la aplicación para abrir directamente esta sección).*

---

## Licencia

Distribuido bajo los términos de la licencia [Apache 2.0](LICENSE).
