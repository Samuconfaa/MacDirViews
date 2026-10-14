<p align="center">
  <a href="README.md">English</a> &nbsp;|&nbsp;
  <a href="README_IT.md">Italiano</a> &nbsp;|&nbsp;
  <strong>Français</strong> &nbsp;|&nbsp;
  <a href="README_ES.md">Español</a> &nbsp;|&nbsp;
  <a href="README_RU.md">Русский</a>
</p>

***

# MacDirViews

Un visualiseur d'espace disque et un outil de nettoyage moderne, rapide et performant pour macOS.

MacDirViews propose un aperçu graphique interactif et intuitif de l'utilisation de votre disque à l'aide de treemaps rectangulaires (squarified treemaps). Il vous permet d'identifier instantanément les fichiers et répertoires volumineux sur vos disques internes, volumes externes ou dossiers spécifiques, rendant l'analyse et la libération d'espace rapides, claires et sûres.

---

## Pourquoi choisir MacDirViews ?

- **Extrêmement rapide et non bloquant** : Développé en Rust avec analyse multi-threadée native. Il parcourt des millions de fichiers en quelques secondes tout en maintenant l'interface graphique parfaitement fluide à 60 FPS.
- **Calcul précis pour APFS** : Mesure l'occupation physique réelle sur le disque plutôt que la simple taille nominale. Il gère avec précision les fichiers fragmentés (sparse files) et évite le double comptage des clones copy-on-write et des liens physiques APFS.
- **Léger et optimisé** : Conçu avec un moteur graphique natif accéléré par le GPU. Faible empreinte mémoire, sans framework web encombrant ni surcharge liée à Electron.
- **Sécurisé par conception** : Intègre des garde-fous empêchant la suppression accidentelle des répertoires et fichiers système essentiels (`/`, `/System`, `/usr`, `/bin`, dossiers racines des utilisateurs). S'intègre directement avec la Corbeille de macOS pour une récupération possible.

---

## Fonctionnalités Principales

- **Treemap Squarified Interactive** : Représente la taille des dossiers et des fichiers sous forme de blocs géométriques proportionnels. Cliquez sur un bloc pour afficher les détails et le pourcentage ; double-cliquez pour explorer un sous-dossier.
- **Classements Top 10 Dossiers et Top 20 Fichiers** : Listes dédiées mettant en évidence les plus gros répertoires de la vue active et les fichiers individuels les plus volumineux de l'ensemble de l'analyse.
- **Filtres et Recherche en Temps Réel** :
  - Recherche textuelle instantanée pour localiser des éléments par nom.
  - Curseur logarithmique de seuil minimal pour masquer les dossiers de taille négligeable.
- **Prise en Charge des Volumes Externes** : Sélection et analyse rapides des disques durs externes, clés USB et partitions secondaires montés dans `/Volumes`.
- **Glisser-Déposer (Drag & Drop)** : Déposez directement n'importe quel dossier depuis le Finder dans la fenêtre pour lancer une analyse immédiate.
- **Intégration Finder et Corbeille** : Clic droit sur n'importe quel élément pour le révéler dans le Finder, le placer dans la Corbeille de macOS ou le supprimer définitivement via une boîte de dialogue de confirmation explicite.

---

## Autorisations macOS (Accès Complet au Disque)

macOS protège les données personnelles des utilisateurs (telles que Mail, Messages, l'historique Safari et les photothèques) par le mécanisme TCC (Transparency, Consent, and Control).

Pour permettre à MacDirViews d'analyser l'intégralité du disque de démarrage sans erreurs de permissions :

1. Ouvrez **Réglages Système** → **Confidentialité et sécurité** → **Accès complet au disque**.
2. Cliquez sur le bouton **+** et ajoutez **MacDirViews** (ou votre binaire compilé).
3. Redémarrez l'application.

*(Vous pouvez également utiliser le bouton **Full Disk Access** situé dans la barre supérieure de l'application pour ouvrir directement ce panneau).*

---

## Installation et Compilation

### Prérequis
- macOS 11.0 (Big Sur) ou version ultérieure
- Apple Silicon (M1/M2/M3/M4) ou Intel x86_64
- Chaîne d'outils Rust (`cargo`) et Xcode Command Line Tools

### Exécution depuis le Code Source

```bash
cargo run --release
```

### Création du Bundle d'Application macOS (.app)

1. Installez l'outil de packaging :
   ```bash
   cargo install cargo-bundle
   ```

2. Générez le paquet `.app` :
   ```bash
   cargo bundle --release
   ```

   L'application sera créée à l'emplacement suivant :
   ```
   target/release/bundle/osx/MacDirViews.app
   ```

3. Copiez-la dans votre dossier Applications :
   ```bash
   cp -R target/release/bundle/osx/MacDirViews.app /Applications/
   ```

> [!NOTE]
> Si macOS Gatekeeper bloque l'application non signée lors du premier lancement, retirez l'attribut de quarantaine via le terminal :
> ```bash
> xattr -cr /Applications/MacDirViews.app
> ```

---

## Licence

Distribué selon les termes spécifiés dans le dépôt.
