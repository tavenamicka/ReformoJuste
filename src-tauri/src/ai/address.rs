//! Forme d'adresse (tutoiement / vouvoiement) : détection déterministe.
//!
//! Le prompt portait la consigne « conserve la forme d'adresse » comme une
//! règle générale, formulée en négatif (« ne convertis JAMAIS »). Les petits
//! modèles (`mistral-small`, `gemma3:4b`) la lâchaient systématiquement au
//! profit du vouvoiement : les noms de registres demandés — « professionnel »,
//! « soutenu » — sont corrélés à « vous » dans les données d'entraînement, et
//! ils apparaissaient cinq fois dans le prompt contre une seule pour la règle.
//!
//! On détecte donc la forme ici, en Rust, pour l'injecter dans le prompt comme
//! un **fait** (« le texte TUTOIE ») plutôt qu'une règle, et pour vérifier la
//! sortie après coup.

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    Tu,
    Vous,
    /// Aucun marqueur, ou autant des deux côtés : on ne tranche pas.
    Unknown,
}

/// Marqueurs non ambigus : pronoms et possessifs de 2ᵉ personne du singulier.
/// La famille « tien / tienne / tiens » est volontairement absente : « tiens »
/// est aussi le verbe tenir à la 1ʳᵉ personne (« je tiens à… »), un faux positif
/// bien plus fréquent que le possessif qu'il détecterait.
const TU_STRONG: &[&str] = &["tu", "te", "toi", "tes"];

/// `ton` et `ta` sont aussi des noms communs (« le ton du message », « ta » en
/// notation musicale) : demi-poids, pour qu'un seul d'entre eux ne l'emporte
/// pas sur un « votre » explicite.
const TU_WEAK: &[&str] = &["ton", "ta"];

const VOUS_STRONG: &[&str] = &["vous", "votre", "vos", "vôtre", "vôtres"];

const STRONG: u32 = 2;
const WEAK:   u32 = 1;

fn classify(token: &str, elided: bool) -> (u32, u32) {
    if token.is_empty() {
        return (0, 0);
    }
    // Élision de « te » : « je t'ai dit », « t'as vu ». Le `t` euphonique
    // (« a-t-il ») s'écrit avec des traits d'union, pas une apostrophe.
    if elided && token == "t" {
        return (STRONG, 0);
    }
    if TU_STRONG.contains(&token) {
        return (STRONG, 0);
    }
    if TU_WEAK.contains(&token) {
        return (WEAK, 0);
    }
    if VOUS_STRONG.contains(&token) {
        return (0, STRONG);
    }
    (0, 0)
}

/// Scores (tutoiement, vouvoiement). Tout caractère non alphabétique sépare
/// deux tokens — y compris le trait d'union, si bien que « toi-même » compte
/// via « toi ».
fn score(text: &str) -> (u32, u32) {
    let (mut tu, mut vous) = (0, 0);
    let lower = text.to_lowercase();
    let mut token = String::new();

    // L'espace ajoutée en fin d'itération vide le dernier token.
    for ch in lower.chars().chain(std::iter::once(' ')) {
        if ch.is_alphabetic() {
            token.push(ch);
            continue;
        }
        let elided = ch == '\'' || ch == '\u{2019}';
        let (t, v) = classify(&token, elided);
        tu   += t;
        vous += v;
        token.clear();
    }
    (tu, vous)
}

pub fn detect(text: &str) -> Address {
    let (tu, vous) = score(text);
    match tu.cmp(&vous) {
        Ordering::Greater => Address::Tu,
        Ordering::Less    => Address::Vous,
        Ordering::Equal   => Address::Unknown, // inclut (0, 0)
    }
}

/// `true` si `candidate` emploie franchement l'autre forme que `expected`.
///
/// Un candidat sans marqueur n'est jamais une violation : une reformulation
/// peut légitimement éviter l'adresse directe (infinitif, tournure
/// impersonnelle). Limite connue : on compare les formes *dominantes*, donc un
/// « vous » isolé noyé dans du tutoiement passe — c'est la conversion en bloc
/// qu'on veut attraper, pas la fuite ponctuelle.
pub fn violates(expected: Address, candidate: &str) -> bool {
    if expected == Address::Unknown {
        return false;
    }
    let found = detect(candidate);
    found != Address::Unknown && found != expected
}

/// Fragment injecté dans les deux prompts. Formulé à l'affirmative, avec la
/// liste explicite des mots attendus : une contrainte opérationnelle tient
/// mieux qu'une interdiction sur les modèles légers.
pub fn prompt_rule(address: Address) -> &'static str {
    match address {
        Address::Tu => "FORME D'ADRESSE — le texte TUTOIE son destinataire. \
Toutes les valeurs renvoyées doivent tutoyer : « tu », « te », « toi », « ton/ta/tes », \
verbes à la 2e personne du singulier. N'écris ni « vous », ni « votre », ni « vos ». \
Un registre professionnel ou soutenu se rend au tutoiement \
(ex. « Aurais-tu l'obligeance de… », « Je te prie de bien vouloir… »).",

        Address::Vous => "FORME D'ADRESSE — le texte VOUVOIE son destinataire. \
Toutes les valeurs renvoyées doivent vouvoyer : « vous », « votre », « vos », \
verbes à la 2e personne du pluriel. N'écris ni « tu », ni « te », ni « toi », ni « ton/ta/tes ». \
Un registre simple ou créatif reste au vouvoiement.",

        Address::Unknown => "FORME D'ADRESSE — reproduis exactement celle du texte d'origine. \
Ne passe pas du tutoiement au vouvoiement ni l'inverse, et n'introduis pas d'adresse \
directe si le texte d'origine n'en contient pas.",
    }
}

/// Consigne ajoutée à la seconde tentative, après une violation détectée.
pub fn retry_note(address: Address, offenders: &[&str]) -> String {
    let expected = match address {
        Address::Tu      => "le TUTOIEMENT",
        Address::Vous    => "le VOUVOIEMENT",
        Address::Unknown => "la forme d'adresse d'origine",
    };
    if offenders.is_empty() {
        format!(
            "ATTENTION — la tentative précédente a changé la forme d'adresse. \
Reprends en respectant strictement {expected}."
        )
    } else {
        format!(
            "ATTENTION — la tentative précédente a changé la forme d'adresse dans : {}. \
Reprends les cinq valeurs en respectant strictement {expected}.",
            offenders.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecte_le_tutoiement() {
        assert_eq!(detect("Tu peux me rappeler demain ?"), Address::Tu);
        assert_eq!(detect("je t'ai envoyé le fichier"), Address::Tu);
        assert_eq!(detect("t'as vu mon message"), Address::Tu);
        assert_eq!(detect("apporte tes notes et viens par toi-même"), Address::Tu);
    }

    #[test]
    fn detecte_le_vouvoiement() {
        assert_eq!(detect("Pourriez-vous me rappeler demain ?"), Address::Vous);
        assert_eq!(detect("J'ai bien reçu votre message et vos documents"), Address::Vous);
    }

    #[test]
    fn aucun_marqueur_reste_indetermine() {
        assert_eq!(detect("Réunion reportée à jeudi."), Address::Unknown);
        assert_eq!(detect(""), Address::Unknown);
    }

    /// « ton » nom commun ne doit pas l'emporter sur un « votre » explicite.
    #[test]
    fn ton_nom_commun_ne_fait_pas_basculer() {
        assert_eq!(detect("le ton de votre message m'a surpris"), Address::Vous);
    }

    #[test]
    fn violation_dans_les_deux_sens() {
        assert!(violates(Address::Tu, "Pourriez-vous confirmer votre présence ?"));
        assert!(violates(Address::Vous, "Tu peux confirmer ta présence ?"));
    }

    #[test]
    fn pas_de_violation_quand_la_forme_est_respectee() {
        assert!(!violates(Address::Tu, "Pourrais-tu confirmer ta présence ?"));
        assert!(!violates(Address::Vous, "Pourriez-vous confirmer votre présence ?"));
    }

    /// Une reformulation sans adresse directe est légitime, pas une violation.
    #[test]
    fn absence_dadresse_directe_nest_pas_une_violation() {
        assert!(!violates(Address::Tu, "Merci de confirmer la présence."));
        assert!(!violates(Address::Vous, "Confirmation de présence souhaitée."));
    }

    /// Forme indéterminée en entrée : on ne peut rien reprocher à la sortie.
    #[test]
    fn source_indeterminee_ne_declenche_jamais_de_reprise() {
        assert!(!violates(Address::Unknown, "Pourriez-vous confirmer ?"));
        assert!(!violates(Address::Unknown, "Tu confirmes ?"));
    }
}
