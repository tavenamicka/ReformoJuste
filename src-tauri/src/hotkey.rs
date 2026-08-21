use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rdev::{listen, Event, EventType, Key};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::config::Config;

#[derive(Default)]
struct State {
    ctrl_held:    bool,
    last_trigger: Option<Instant>,
}

#[derive(Clone, Serialize)]
struct TextPayload {
    text: String,
}

/// Géométrie de la popup, figée au démarrage depuis la config.
#[derive(Clone, Copy)]
struct Geometry {
    width:    i32,
    height:   i32,
    offset_x: i32,
    offset_y: i32,
}

/// Spawns the global keyboard listener thread.
///
/// **Contrainte forte** : sous Windows, `rdev::listen` installe un hook bas
/// niveau (`WH_KEYBOARD_LL`). Tant que le callback n'a pas rendu la main, la
/// file d'entrée clavier de **tout le système** est bloquée ; au-delà de
/// `LowLevelHooksTimeout` (300 ms par défaut) Windows retire silencieusement le
/// hook et le raccourci ne répond plus du tout jusqu'au redémarrage de l'app.
///
/// Le callback ne fait donc que de l'arithmétique sur un `Instant` et pousse un
/// message dans un canal. Tout le reste — interrogation de la fenêtre Tauri,
/// simulation du Ctrl+C, lecture du presse-papier — se passe dans le worker.
pub fn start(app: AppHandle, config: Arc<Config>) {
    let ms = config.hotkey.double_press_ms;
    let geo = Geometry {
        width:    config.popup.width  as i32,
        height:   config.popup.height as i32,
        offset_x: config.popup.offset_x,
        offset_y: config.popup.offset_y,
    };

    let (tx, rx) = mpsc::channel::<()>();

    // Worker : sérialise les déclenchements, hors du hook clavier.
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            // Coalesce les déclenchements accumulés pendant le traitement du
            // précédent (double-frappe nerveuse) : un seul cycle suffit.
            while rx.try_recv().is_ok() {}
            toggle_popup(&app, geo);
        }
    });

    std::thread::spawn(move || {
        let state = Arc::new(Mutex::new(State::default()));

        // rdev::listen is blocking; it runs its own Windows message loop internally.
        if let Err(e) = listen(move |event| {
            on_event(&event, &state, &tx, ms);
        }) {
            eprintln!("[hotkey] listener error: {:?}", e);
        }
    });
}

/// Exécuté dans le hook clavier : doit rester en dizaines de microsecondes.
fn on_event(event: &Event, state: &Arc<Mutex<State>>, tx: &Sender<()>, ms: u64) {
    let mut s = match state.lock() {
        Ok(s)  => s,
        Err(_) => return,
    };

    match &event.event_type {
        EventType::KeyPress(Key::ControlLeft)
        | EventType::KeyPress(Key::ControlRight) => {
            s.ctrl_held = true;
        }
        EventType::KeyRelease(Key::ControlLeft)
        | EventType::KeyRelease(Key::ControlRight) => {
            s.ctrl_held = false;
        }

        // Double Ctrl+Space detection
        EventType::KeyPress(Key::Space) if s.ctrl_held => {
            let double = s.last_trigger
                .map(|t| t.elapsed().as_millis() < ms as u128)
                .unwrap_or(false);

            if double {
                s.last_trigger = None;
                drop(s);
                let _ = tx.send(());   // le worker prend le relais
            } else {
                s.last_trigger = Some(Instant::now());
            }
        }
        _ => {}
    }
}

/// Bascule la popup : masquée → capture + affichage, visible → masquée.
fn toggle_popup(app: &AppHandle, geo: Geometry) {
    let Some(window) = app.get_window("popup") else { return };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    // 1. Simulate Ctrl+C in the currently-focused window
    if let Err(e) = crate::clipboard::simulate_copy() {
        eprintln!("[hotkey] simulate_copy failed: {}", e);
    }

    // 2. Read clipboard
    let text = crate::clipboard::read()
        .unwrap_or_else(|_| String::from("(aucun texte)"));

    // 3. Cursor + monitor containing cursor (multi-monitor safe)
    let cursor  = crate::cursor::get_position();
    let monitor = crate::cursor::get_monitor_rect(cursor);

    // 4. Safe popup position
    let (x, y) = crate::cursor::clamp_popup(
        cursor, monitor,
        (geo.width, geo.height),
        (geo.offset_x, geo.offset_y),
    );

    // 5. Move, show, emit
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("text-ready", TextPayload { text });
}
