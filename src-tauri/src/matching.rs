//! Zuordnung eines Druckauftrags zu einem Auftrag – unabhängig vom Anbieter (Bambu, später andere Drucker-Dienste).
//! Verglichen werden Namen: Druckauftrag ↔ Dateien und Titel der offenen Aufträge, plus Auftragskennung `A0004`.

use crate::model::OrderFiles;

/// Dateiname oder Druckauftrag vergleichbar machen: ohne Endung und „_plate_N“, nur Buchstaben/Ziffern.
pub fn normalize(name: &str) -> String {
    let mut s = name.trim().to_lowercase();
    for ext in [".gcode.3mf", ".3mf", ".gcode", ".stl", ".step", ".stp", ".obj"] {
        if let Some(x) = s.strip_suffix(ext) {
            s = x.to_string();
            break;
        }
    }
    if let Some(i) = s.rfind("_plate_") {
        if s[i + 7..].chars().all(|c| c.is_ascii_digit()) {
            s.truncate(i);
        }
    }
    let s: String = s.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Wörter eines Namens ohne Auftragskennung (`a0004`), für den Vergleich.
fn words(name: &str) -> Vec<String> {
    normalize(name)
        .split(' ')
        .filter(|w| !w.is_empty() && !is_tag_word(w))
        .map(str::to_string)
        .collect()
}

fn is_tag_word(w: &str) -> bool {
    w.len() >= 5 && w.starts_with('a') && w[1..].chars().all(|c| c.is_ascii_digit())
}

/// Ähnlichkeit zweier Namen: 3 = gleich, 2 = alle Wörter des kürzeren kommen im längeren vor, 0 = nein.
/// Der kürzere braucht mindestens zwei Wörter oder ein Wort mit 4+ Zeichen („Halter“ ≠ „Kabelhalter“).
pub fn similarity(a: &str, b: &str) -> u8 {
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    if a == b {
        return 3;
    }
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    let strong = short.len() >= 2 || short[0].chars().count() >= 4;
    if strong && short.iter().all(|w| long.contains(w)) {
        2
    } else {
        0
    }
}

/// Auftragskennung aus einem Druckauftrag lesen: „A0004 benchy_plate_1“ → 4.
pub fn order_number_in(task: &str) -> Option<i64> {
    normalize(task).split(' ').find(|w| is_tag_word(w)).and_then(|w| w[1..].parse().ok())
}

/// Ergebnis der Zuordnung eines Druckauftrags
pub struct Match<'a> {
    pub order: &'a OrderFiles,
    pub file: Option<&'a str>,
    /// manuell | nummer | datei | titel
    pub by: &'static str,
}

/// Offenen Auftrag zu einem Druckauftrag finden, in dieser Reihenfolge:
///
/// 1. von Hand festgelegt
/// 2. Auftragskennung im Namen (`A0004 …`)
/// 3. beste Übereinstimmung von Dateiname oder Titel; bei Gleichstand gewinnt der Auftrag in
///    „Druckt“/„In Studio“, dann der mit dem neuesten Druck, dann der neueste Auftrag
pub fn find_order<'a>(orders: &'a [OrderFiles], task: &str, assigned: Option<&str>) -> Option<Match<'a>> {
    let best_file = |o: &'a OrderFiles| -> Option<(&'a str, u8)> {
        o.files.iter().map(|f| (f.as_str(), similarity(f, task))).filter(|x| x.1 > 0).max_by_key(|x| x.1)
    };
    if let Some(o) = assigned.and_then(|id| orders.iter().find(|o| o.order_id == id)) {
        return Some(Match { order: o, file: best_file(o).map(|x| x.0), by: "manuell" });
    }
    if let Some(o) = order_number_in(task).and_then(|n| orders.iter().find(|o| o.number == n)) {
        return Some(Match { order: o, file: best_file(o).map(|x| x.0), by: "nummer" });
    }
    let status_bonus = |s: &str| match s {
        "druckt" => 3,
        "in_studio" => 2,
        "vorbereiten" | "nacharbeit" => 1,
        _ => 0,
    };
    orders
        .iter()
        .filter_map(|o| {
            let file = best_file(o);
            let title = similarity(&o.title, task);
            // Datei zählt mehr als Titel
            let (score, by, f) = match file {
                Some((f, sc)) if sc * 2 >= title => (sc as i32 * 2, "datei", Some(f)),
                _ if title > 0 => (title as i32, "titel", None),
                _ => return None,
            };
            Some((score, status_bonus(&o.status), o.last_run_at.clone(), o.number, o, by, f))
        })
        .max_by(|a, b| (a.0, a.1, &a.2, a.3).cmp(&(b.0, b.1, &b.2, b.3)))
        .map(|(_, _, _, _, o, by, f)| Match { order: o, file: f, by })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen_vergleichen() {
        assert_eq!(normalize("Suporte_de_Caneta_Samurai_plate_2"), "suporte de caneta samurai");
        assert_eq!(normalize("figuren.gcode.3mf"), "figuren");
        assert!(similarity("Suporte de Caneta Samurai.3mf", "Suporte_de_Caneta_Samurai_plate_2") > 0);
        assert!(similarity("Benchy.stl", "benchy_plate_1") > 0);
        assert_eq!(similarity("Vase.3mf", "Schlüsselanhänger"), 0);
        assert_eq!(similarity("ab.3mf", "abc"), 0, "zu kurz für Teiltreffer");
        assert_eq!(similarity("Halter.stl", "Kabelhalter"), 0, "nur ganze Wörter");
    }

    fn of(id: &str, number: i64, title: &str, status: &str, files: &[&str], run: Option<&str>) -> OrderFiles {
        OrderFiles {
            order_id: id.into(), number, title: title.into(), status: status.into(),
            files: files.iter().map(|f| f.to_string()).collect(), last_run_at: run.map(Into::into),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn auftrag_finden() {
        let orders = vec![
            of("1", 3, "Benchy für Oma", "eingang", &["3DBenchy.stl"], None),
            of("2", 6, "Figuren", "druckt", &["figuren.gcode.3mf"], Some("2026-11-16T10:00:00Z")),
        ];
        let m = find_order(&orders, "figuren_plate_1", None).unwrap();
        assert_eq!((m.order.number, m.file, m.by), (6, Some("figuren.gcode.3mf"), "datei"));
        let m = find_order(&orders, "3DBenchy", None).unwrap();
        assert_eq!((m.order.number, m.file), (3, Some("3DBenchy.stl")));
        assert!(find_order(&orders, "Kabelhalter", None).is_none());
        // von Hand festgelegt schlägt alles
        assert_eq!(find_order(&orders, "figuren_plate_1", Some("1")).unwrap().by, "manuell");
    }

    #[test]
    fn mehrdeutige_namen_und_kennung() {
        // drei Aufträge mit derselben Datei (wie #3, #4, #5 mit 3dbenchy.stl)
        let orders = vec![
            of("a", 3, "3D BENCHY", "eingang", &["3dbenchy.stl"], None),
            of("b", 4, "3D BENCHY", "in_studio", &["3dbenchy.stl"], None),
            of("c", 5, "3D BENCHY", "eingang", &["A0005 3dbenchy.stl"], None),
        ];
        // Kennung im Druckauftrag: eindeutig
        let m = find_order(&orders, "A0005 3dbenchy_plate_1", None).unwrap();
        assert_eq!((m.order.number, m.by, m.file), (5, "nummer", Some("A0005 3dbenchy.stl")));
        assert_eq!(order_number_in("A0004 3dbenchy_plate_1"), Some(4));
        assert_eq!(order_number_in("3dbenchy_plate_1"), None);
        // ohne Kennung: der Auftrag „In Studio“ gewinnt statt einfach der erste
        assert_eq!(find_order(&orders, "3dbenchy_plate_1", None).unwrap().order.number, 4);
        // Kennung eines nicht offenen Auftrags: Rückfall auf den Namen
        assert_eq!(find_order(&orders, "A0099 3dbenchy", None).unwrap().order.number, 4);
    }

    #[test]
    fn woerter_statt_teilzeichen() {
        assert_eq!(similarity("Halter.stl", "Kabelhalter_plate_1"), 0, "Teilwort zählt nicht");
        assert_eq!(similarity("Kabel Halter.stl", "kabel_halter_lang_plate_1"), 2);
        assert_eq!(similarity("A0004 Vase.3mf", "vase_plate_2"), 3, "Kennung stört den Vergleich nicht");
        assert_eq!(similarity("ab.3mf", "ab cd"), 0, "zu schwach");
    }
}
