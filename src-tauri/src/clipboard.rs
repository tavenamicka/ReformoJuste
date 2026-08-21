use anyhow::Result;
use arboard::Clipboard;
use std::thread;
use std::time::{Duration, Instant};

/// Le presse-papier Windows est une ressource exclusive : une autre application
/// (Office, navigateur, gestionnaire d'historique…) peut le tenir ouvert au
/// moment où on l'ouvre. Un échec ponctuel se traduisait par « (aucun texte) ».
const CLIPBOARD_ATTEMPTS: u32 = 3;
const CLIPBOARD_RETRY_MS: u64 = 25;

fn with_clipboard<T>(mut f: impl FnMut(&mut Clipboard) -> Result<T>) -> Result<T> {
    let mut last = anyhow::anyhow!("clipboard: aucune tentative");
    for attempt in 0..CLIPBOARD_ATTEMPTS {
        match Clipboard::new().map_err(anyhow::Error::from).and_then(|mut cb| f(&mut cb)) {
            Ok(v)  => return Ok(v),
            Err(e) => {
                last = e;
                if attempt + 1 < CLIPBOARD_ATTEMPTS {
                    thread::sleep(Duration::from_millis(CLIPBOARD_RETRY_MS));
                }
            }
        }
    }
    Err(last)
}

pub fn read() -> Result<String> {
    with_clipboard(|cb| Ok(cb.get_text()?))
}

pub fn write(text: &str) -> Result<()> {
    with_clipboard(|cb| Ok(cb.set_text(text)?))
}

// ── Simulation clavier ────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn send(ev: rdev::EventType) -> Result<()> {
    rdev::simulate(&ev).map_err(|_| anyhow::anyhow!("rdev simulate failed"))
}

/// Numéro de séquence du presse-papier : incrémenté par Windows à chaque
/// écriture. Bien moins coûteux que de relire le contenu, et fiable même quand
/// on recopie un texte identique.
#[cfg(target_os = "windows")]
fn clipboard_sequence() -> u32 {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { GetClipboardSequenceNumber() }
}

/// Simulates Ctrl+C in the currently-focused window so the selection is copied.
///
/// Attend que le presse-papier soit réellement mis à jour au lieu de dormir un
/// délai fixe : l'ouverture de la popup gagne ~120 ms dans le cas courant, tout
/// en tolérant les applications lentes jusqu'à `MAX_WAIT`.
#[cfg(target_os = "windows")]
pub fn simulate_copy() -> Result<()> {
    use rdev::{EventType, Key};

    const MAX_WAIT: Duration = Duration::from_millis(300);
    const POLL:     Duration = Duration::from_millis(10);
    /// Le numéro de séquence bouge à la fermeture du presse-papier ; on laisse
    /// une marge minime avant de lire.
    const SETTLE:   Duration = Duration::from_millis(15);

    let before = clipboard_sequence();

    send(EventType::KeyPress(Key::ControlLeft))?;
    send(EventType::KeyPress(Key::KeyC))?;
    thread::sleep(Duration::from_millis(30));
    send(EventType::KeyRelease(Key::KeyC))?;
    send(EventType::KeyRelease(Key::ControlLeft))?;

    let deadline = Instant::now() + MAX_WAIT;
    while Instant::now() < deadline {
        if clipboard_sequence() != before {
            thread::sleep(SETTLE);
            return Ok(());
        }
        thread::sleep(POLL);
    }
    // Rien n'a été copié (application sans Ctrl+C) : on repart sur le contenu
    // existant du presse-papier, comme avant.
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn simulate_copy() -> Result<()> {
    Ok(())
}

/// Simulates Ctrl+V in the currently-focused window to paste clipboard content.
#[cfg(target_os = "windows")]
pub fn simulate_paste() -> Result<()> {
    use rdev::{EventType, Key};

    send(EventType::KeyPress(Key::ControlLeft))?;
    send(EventType::KeyPress(Key::KeyV))?;
    thread::sleep(Duration::from_millis(30));
    send(EventType::KeyRelease(Key::KeyV))?;
    send(EventType::KeyRelease(Key::ControlLeft))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn simulate_paste() -> Result<()> {
    Ok(())
}
