//! Démarrage automatique avec Windows.
//!
//! Passe par la clé `Run` de l'utilisateur courant (`HKEY_CURRENT_USER`) :
//! aucun droit administrateur requis, et le réglage suit le profil utilisateur
//! plutôt que la machine — cohérent avec une application portable.
//!
//! Le chemin de l'exécutable est réenregistré au démarrage s'il a changé, pour
//! que déplacer le dossier portable ne laisse pas une entrée morte.

use anyhow::Result;

#[cfg(target_os = "windows")]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Nom de la valeur dans la clé Run. Ne jamais le changer sans nettoyer
/// l'ancien : deux valeurs différentes lanceraient l'app deux fois.
#[cfg(target_os = "windows")]
const VALUE_NAME: &str = "ReformoJuste";

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_key() -> Result<winreg::RegKey> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)?;
    Ok(key)
}

/// Ligne de commande à enregistrer. Les guillemets sont indispensables : le
/// dossier portable peut vivre dans un chemin contenant des espaces.
#[cfg(target_os = "windows")]
fn command_line() -> Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

#[cfg(target_os = "windows")]
fn registered() -> Option<String> {
    run_key().ok()?.get_value::<String, _>(VALUE_NAME).ok()
}

#[cfg(target_os = "windows")]
pub fn is_enabled() -> bool {
    registered().is_some()
}

#[cfg(target_os = "windows")]
fn enable() -> Result<()> {
    run_key()?.set_value(VALUE_NAME, &command_line()?)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable() -> Result<()> {
    match run_key()?.delete_value(VALUE_NAME) {
        Ok(())                                                  => Ok(()),
        // Déjà absente : l'état voulu est atteint.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound       => Ok(()),
        Err(e)                                                  => Err(e.into()),
    }
}

/// Réaligne l'entrée existante si l'exécutable a été déplacé.
/// Sans effet si le démarrage automatique n'est pas activé.
#[cfg(target_os = "windows")]
pub fn sync_path() {
    let Some(current) = registered() else { return };
    let Ok(expected) = command_line() else { return };

    // Les chemins Windows sont insensibles à la casse.
    if !current.eq_ignore_ascii_case(&expected) {
        match enable() {
            Ok(())  => eprintln!("[autostart] chemin réaligné : {current} → {expected}"),
            Err(e)  => eprintln!("[autostart] réalignement impossible : {e}"),
        }
    }
}

// ── Autres plateformes ────────────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub fn is_enabled() -> bool { false }

#[cfg(not(target_os = "windows"))]
fn enable() -> Result<()> { anyhow::bail!("démarrage automatique non pris en charge sur cette plateforme") }

#[cfg(not(target_os = "windows"))]
fn disable() -> Result<()> { Ok(()) }

#[cfg(not(target_os = "windows"))]
pub fn sync_path() {}

// ── API commune ───────────────────────────────────────────────────────────────

pub fn set(enabled: bool) -> Result<()> {
    if enabled { enable() } else { disable() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn command_line_est_entre_guillemets() {
        // Un chemin portable non protégé casserait au premier espace.
        let cmd = command_line().expect("chemin de l'exécutable");
        assert!(cmd.starts_with('"') && cmd.ends_with('"'), "non protégé : {cmd}");
    }

    /// Aller-retour sur la vraie clé Run. L'état d'origine est restauré en fin
    /// de test, y compris si l'utilisateur avait déjà activé le démarrage auto.
    #[test]
    fn activation_desactivation_aller_retour() {
        let initial = registered();

        set(true).expect("activation");
        assert!(is_enabled(), "devrait être actif après set(true)");
        let expected = command_line().expect("chemin");
        assert_eq!(registered().as_deref(), Some(expected.as_str()));

        // Ré-activer une entrée déjà présente doit rester sans effet de bord.
        set(true).expect("activation idempotente");
        assert_eq!(registered().as_deref(), Some(expected.as_str()));

        set(false).expect("désactivation");
        assert!(!is_enabled(), "devrait être inactif après set(false)");

        // Supprimer une entrée absente ne doit pas remonter d'erreur.
        set(false).expect("désactivation idempotente");

        // Restauration.
        if let Some(value) = initial {
            run_key().unwrap().set_value(VALUE_NAME, &value).unwrap();
        }
    }
}
