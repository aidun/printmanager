//! Slicer als Anbindung: Programm finden und eine Datei darin öffnen, seine Filament-Datei lesen,
//! gesliced Projekte lesen. Startet nur das Programm – Drucke startet die App nie (AGENTS.md Regel 1)
//! und sie schreibt nie in Dateien des Slicers (Regel 3).

use crate::error::{AppError, Result};
use crate::model::StudioSpool;
use crate::threemf::Info;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Auftrag zum Slicen im Hintergrund (nur zur Kalkulation)
#[derive(Debug, Clone)]
pub struct SliceJob {
    /// Modell (STL, 3MF, STEP)
    pub model: PathBuf,
    /// Bambu-Druckermodell, z.B. „Bambu Lab H2C 0.4 nozzle“
    pub printer: String,
    /// Druckplatte, z.B. „Textured PEI Plate“
    pub bed_type: String,
    pub process: PathBuf,
    /// ein Filament-Preset je Filament im Modell
    pub filaments: Vec<PathBuf>,
    /// Slot je eingelesener Datei (`--load-filament-ids`; Bambu Studio erwartet einen Wert je Datei,
    /// nicht je Objekt); leer = wie im Modell
    pub filament_ids: Vec<i64>,
    /// Objekte neu anordnen (nur bei einzelnen Modellen wie STL; ein Projekt behält seine Platten)
    pub arrange: bool,
    /// Arbeitsordner für Profile und Ergebnis
    pub work_dir: PathBuf,
}

pub trait SlicerAdapter: Send + Sync {
    /// Anzeigename des Programms, z.B. „Bambu Studio“
    fn program_name(&self) -> &'static str;

    /// Übliche Installationsorte auf diesem Betriebssystem
    fn program_candidates(&self) -> &'static [&'static str];

    /// Programm finden: eingestellter Pfad (falls vorhanden), sonst die üblichen Orte.
    fn find_program(&self, configured: Option<&str>) -> Option<PathBuf> {
        if let Some(p) = configured.filter(|p| !p.trim().is_empty()) {
            return Some(PathBuf::from(p)).filter(|p| p.exists());
        }
        self.program_candidates().iter().map(PathBuf::from).find(|p| p.exists())
    }

    /// Datei im Programm öffnen.
    fn open(&self, configured: Option<&str>, file: &Path) -> Result<()> {
        let program = self.find_program(configured).ok_or_else(|| {
            AppError::Invalid(format!("{} wurde nicht gefunden. Pfad in den Einstellungen angeben.", self.program_name()))
        })?;
        let mut cmd = if cfg!(target_os = "macos") {
            let mut c = Command::new("open");
            c.arg("-a").arg(&program).arg(file);
            c
        } else {
            let mut c = Command::new(&program);
            c.arg(file);
            c
        };
        cmd.spawn().map(|_| ()).map_err(|e| AppError::Invalid(format!("{} startet nicht: {e}", self.program_name())))
    }

    /// Standardort der Filament-Datei des Slicers (falls er eine hat)
    fn default_spools_file(&self, system_data_dir: &Path) -> Option<PathBuf>;

    /// Filament-Datei lesen; `None`, wenn es sie nicht gibt.
    fn read_spools(&self, file: &Path) -> Result<Option<Vec<StudioSpool>>>;

    /// Ein (gesliced) Projekt lesen: Druckzeit, Gramm, Filamente, Vorschau.
    fn read_project(&self, file: &Path) -> Result<Info>;

    /// Ohne Oberfläche slicen und das gesliced Projekt zurückgeben (nur Kalkulation, nie ein Druck).
    fn slice(&self, _configured: Option<&str>, _job: &SliceJob) -> Result<PathBuf> {
        Err(AppError::Invalid(format!("{} kann nicht im Hintergrund slicen.", self.program_name())))
    }
}
