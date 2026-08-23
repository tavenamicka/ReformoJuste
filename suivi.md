# Suivi UX/Perf — ReformoJuste Correcteur Portable

**Date** : 2026-06-27  
**Scope** : Refonte UX + optimisation du code frontend (Tauri popup 446×286 px)

---

## Phase 1 — P0 (Hiérarchie UX critiques)

### Inversion des boutons (affordance)
- ✅ `Remplacer` → **action primaire** (plein/accent, positionné à droite)
- ✅ `Copier` → **action secondaire** (outline)
- **Fichiers** : `src/style.css` (CSS des boutons), `src/index.html` (ordre HTML)
- **Impact** : La CTA naturelle est visuellement claire (l'utilisateur reprend son app immédiatement après remplacement)

### Auto-hide après remplacement
- ✅ `appWindow.hide()` déclenché après succès de `replace_text`
- **Fichier** : `src/main.js` (listener `elReplace.click`)
- **Impact** : Fermeture automatique = fin du geste sans action supplémentaire

### Diff visuel sur l'onglet Correction
- ✅ Stockage du texte original à capture (`originalText`)
- ✅ Diff mot-à-mot (LCS) → `<ins>` (vert) / `<del>` (rouge barré) / neutre (conservé)
- ✅ HTML échappé (`escapeHtml`) → pas d'injection possible
- **Fichiers** : `src/main.js` (fonction `diffHtml`), `src/style.css` (styles `ins/del`)
- **Impact** : L'utilisateur voit *ce qui change* sans relire => confiance + rapidité

---

## Phase 2 — P1 (Divulgation progressive)

### Compteur de corrections + état « Aucune faute »
- ✅ Diff compte les groupes de mots modifiés contigus (1 groupe = 1 correction)
- ✅ Statut affiché sur l'onglet Correction uniquement : `"3 corrections"` ou `"✓ Aucune faute détectée"`
- ✅ Statut en `aria-live="polite"` (annoncé au lecteur d'écran)
- **Fichier** : `src/main.js` (retour `{ html, changes }` du diff), `src/index.html` (balise `#result-status`)
- **Impact** : Feedback immédiat sans devoir relire ; les cas « zéro faute » ne sont plus muets

### Repli des reformulations
- ✅ Seul **Correction** visible au premier plan
- ✅ Déclencheur **« Reformuler ▾ »** pour déployer 5 tons (Simple/Pro/Soutenu/Court/Créatif)
- ✅ Panneau replié par défaut (économie d'espace vertical)
- ✅ S'ouvre auto au clic sur un ton, se referme à nouvelle capture
- ✅ Désactivé pendant le chargement (`partial`) jusqu'à ce que les résultats arrivent
- **Fichiers** : `src/index.html` (structure 2 rangées), `src/main.js` (`setReformOpen`), `src/style.css` (`.tab-row`, `.tab-toggle`)
- **Impact** : 90 % des usages (corriger) ne paient pas la complexité des 10 % (reformuler)

---

## Phase 3 — P2 (Accessibilité + discoverabilité)

### Pattern ARIA tablist complet
- ✅ `role="tablist"` sur `#tabs`, `role="tab"` sur chaque onglet
- ✅ `aria-selected` synchronisé à chaque changement
- ✅ `role="tabpanel"` + `aria-label` sur la zone de résultat
- ✅ **Roving tabindex** : seul l'onglet actif en `tabindex=0`, les autres en `-1` → traversée clavier optimisée
- ✅ Navigation **flèches ←/→** entre onglets visibles, **Home/End** aux extrêmes, avec activation automatique
- ✅ Les onglets repliés (reformulations masquées) ne sont jamais cibles de navigation
- **Fichier** : `src/main.js` (roving tabindex, handler clavier flèches)
- **Impact** : Lecteur d'écran annonce correctement la structure ; clavier confortable

### Contraste rehaussé
- ✅ `--muted` : `#6c7086` (3.4:1, sous AA) → `#9399b2` (4.7:1, AA)
- **Fichier** : `src/style.css` (variable CSS)
- **Impact** : Texte inactif, labels, hints lisibles pour malvoyants

### Raccourcis visibles
- ✅ Pied de page discret : `"⏎ Remplacer · Ctrl 1–6 onglets · Échap fermer"`
- ✅ Tooltips `title` sur les boutons (ex: `"Remplacer (Entrée)"`)
- ✅ Raccourci **Entrée = Remplacer** câblé (ignoré si focus sur bouton)
- **Fichiers** : `src/index.html` (`#hints`, `title=`), `src/main.js` (listener `Enter`)
- **Impact** : Discoverabilité des shortcuts + workflow ultra-rapide

### Focus clavier visible
- ✅ `:focus-visible` → contour accent 2px (cohérent avec palette Catppuccin)
- ✅ Contour interne sur `#result-text` (pour qu'il soit lisible sur contenu)
- **Fichier** : `src/style.css`
- **Impact** : Navigation clavier non-invisible (défaut des thèmes sombres natifs)

---

## Phase 4 — Complément ARIA

### Roving tabindex + Navigation flèches
- ✅ Seul l'onglet actif entre dans l'ordre de tabulation
- ✅ ←/→ naviguent entre onglets visibles (les repliés sont filtrés)
- ✅ Home/End vont au premier/dernier onglet visible
- ✅ Bouclage : depuis le dernier, → va au premier
- **Fichier** : `src/main.js` (listener `keydown` sur `#tabs`, `const tabs = TABS.filter(...)`)
- **Impact** : Pattern accessible confirmant la recommandation WAI-ARIA

---

## Phase 5 — Refactorisation (Performance, Complexité, Lisibilité, Sécurité)

### Performances
- ✅ **Diff optimisé** : suppression des préfixes/suffixes communs **avant** le LCS  
  - Matrice DP réduite à la zone réellement modifiée (gros gain sur sélections longues)
  - Exemple : 1000 mots changeant 5 mots → DP 10×10 au lieu de 1000×1000
- ✅ **Onglets cachés une seule fois** : `const TABS = [...]` au lieu de `document.querySelectorAll('.tab')` répété
- ✅ **Listeners consolidés** : tab switching par délégation (1 listener) au lieu de N
- **Fichier** : `src/main.js` (refonte complète)

### Complexité réduite
- ✅ Triplet `active/aria-selected/tabindex` dupliqué 4 fois → fonction `markActiveTab()`
- ✅ Patch common « clear pending + enable » dupliqué 2 fois → `enableAllTabs()`
- ✅ Navigation ternaire chaînée au lieu de `if/else if/else`
- ✅ `setStatus` : toggles au lieu de branches explicites
- ✅ `text-ready` : synchrone (l'async n'avait aucun `await`)

### Redondances éliminées
- ✅ `TAB_KEYS` : dérivé du DOM (`TABS.map(t => t.dataset.key)`) au lieu de liste codée en dur → reste synchro
- ✅ Référence à `correctionTab` pour éviter d'identifier « correction » par comparaison chaîne
- ✅ Une seule fonction pour marquer l'onglet actif (classe + ARIA + tabindex)

### Mauvaises pratiques corrigées
- ✅ `parseInt(e.key)` → `Number(e.key)` (pas de radix = piège)
- ✅ Bornage de `Ctrl+1–6` : pas codé en dur `6`, mais `TAB_KEYS.length`
- ✅ `document.querySelector(`.tab[data-key="${key}"]`)?.click()` → index array direct

### Lisibilité
- ✅ Sections réorganisées logiquement : États → Onglets → Diff → Boutons → Interactions → Fenêtre → Raccourcis → Backend
- ✅ Commentaires blocs cohérents en français (ex: `// ── Diff & rendu ────`)
- ✅ Noms explicites : `showPartial()`, `showFull()`, `setReformOpen()`, `markActiveTab()`
- ✅ Commentaire d'intention sur le diff LCS (optimisation préfixe/suffixe)

### Sécurité renforcée
- ✅ Seul `innerHTML` reste sur la branche correction, alimentée **exclusivement** par `escapeHtml` (y compris préfixe/suffixe)
- ✅ Texte backend IA ne peut pas injecter de balise (HTML/JS bloqué)
- ✅ Reformulations restent en `textContent` (inerte)
- ✅ Commentaire ajouté à l'endroit sensible

---

## Fichiers modifiés

| Fichier | Changes |
|---------|---------|
| `src/index.html` | Structure onglets/repli, statut, pied raccourcis, ARIA complètes, tabindex |
| `src/style.css` | Inversion boutons, diff styles, tab-rows, toggle, contraste, focus, pied |
| `src/main.js` | Refonte pour perf/complexité/redondances + diff optimisé + ARIA roving |

---

## Avant/Après (estimé)

| Métrique | Avant | Après | Gain |
|----------|-------|-------|------|
| Temps diff (1000 mots) | ~50ms | ~2ms | **-96%** |
| Listeners onglets | 6 | 1 | **-83%** |
| Affordance CTA | outline | plein | **fixée** |
| Access clavier | ✅ Ctrl+1–6 | ✅ +←/→ Home/End | **+100%** |

---

## Phase 6 — Nettoyage post-refacto

### Suppression du drag redondant
- ✅ Listener `mousedown` sur `header` → `appWindow.startDragging()` supprimé
- ✅ Drag géré uniquement via `data-tauri-drag-region` (déclaratif, idiomatique Tauri)
- ✅ Comportement utilisateur identique, code simplifié
- **Fichier** : `src/main.js` (-5 lignes, -1 listener)
- **Impact** : Une seule source de vérité pour le drag ; moins de code à maintenir

---

**Build validé** : ✅ Oui (2026-06-27)  
**Prêt pour commit** : ✅ Oui

---

# Phase 7 — Audit perf & fiabilité (2026-08-21)

**Constat utilisateur** : ralentissements, et parfois l'application ne répond pas.  
**Scope** : backend Rust (`src-tauri/`) + frontend, version dev et version portable.

## Mesures de référence (poste de dev, avant correctifs)

| Élément | Mesure | Conséquence |
|---|---|---|
| Ollama `gemma3:4b` **à froid** | **20,7 s** (dont 19,4 s de chargement) | Ollama décharge après 5 min d'inactivité → usage sporadique = quasi toujours à froid |
| Ollama à chaud | 1,3 s | Inférence réelle négligeable |
| Mistral API | 1,5 – 2,1 s | Sain |
| JVM LanguageTool | **8,2 s de démarrage, 875 Mo de RAM** | Lancée à **chaque** démarrage en mode `auto`, alors qu'elle n'est que le 3ᵉ maillon du repli |
| Timeout HTTP | **aucun** | Une requête bloquée = popup qui tourne indéfiniment |

## P0 — Causes directes des blocages

### Timeouts HTTP absents
- ✅ `reqwest::Client` sans timeout (défaut = infini) sur les 3 providers → clients partagés avec timeouts explicites : 60 s (IA), 20 s (LanguageTool), 3 s (sondes)
- ✅ Bonus : les clients étaient reconstruits à chaque requête, refaisant le handshake TLS complet (~300 ms mesurés). Ils sont désormais des `LazyLock` clonés → pool de connexions partagé
- **Fichiers** : `ai/mod.rs`, `ai/mistral.rs`, `ai/local.rs`, `ai/languagetool.rs`

### Appels Tauri dans le hook clavier bas niveau
- ✅ `app.get_window()` + `is_visible()` s'exécutaient **dans** le callback `rdev` — c'est-à-dire dans un hook `WH_KEYBOARD_LL`, qui bloque la file d'entrée clavier de tout le système et que Windows **retire silencieusement** au-delà de `LowLevelHooksTimeout` (300 ms)
- ✅ Le callback ne fait plus qu'un calcul sur `Instant` + un `send()` sur un canal ; un thread worker persistant traite le reste
- ✅ Coalescence des déclenchements accumulés (double-frappe nerveuse)
- **Fichier** : `src-tauri/src/hotkey.rs`
- **Impact** : cause la plus probable des ralentissements de frappe **et** du raccourci qui cesse de répondre

### Ollama rechargé à chaque fois
- ✅ `keep_alive: "30m"` sur `/api/generate` (défaut Ollama : 5 min) — vérifié : `ollama ps` affiche « 29 minutes from now »
- ✅ Préchargement au démarrage (prompt vide, 4,4 s) quand Ollama est le provider retenu
- ✅ `num_predict: 512` — borne un modèle qui part en boucle
- **Fichier** : `ai/local.rs`
- **Impact** : ~20 s → ~1,3 s sur le chemin local

### JVM LanguageTool lancée pour rien
- ✅ Démarrage **à la demande** (`LtProcess::ensure_started`) : en mode `auto` la JVM reste dormante tant que Mistral ou Ollama répondent
- ✅ Démarrage immédiat conservé pour les modes `hybrid` et `languagetool`, qui en dépendent à chaque requête
- ✅ La JVM est tuée sur `RunEvent::Exit`, plus seulement via le menu du tray (plus de JVM orpheline à 875 Mo)
- **Fichier** : `src-tauri/src/main.rs`
- **Impact vérifié** : au repos, **1229 Mo → 354 Mo** (−71 %)

### `{}` renvoyé par le modèle = échec total
- ✅ `AiResult` en `#[serde(default)]` : une clé manquante ne fait plus échouer tout le parsing
- ✅ `parse_response` rejette explicitement une `correction` vide (observé en test : une réponse Ollama à 2 tokens)
- **Fichier** : `ai/mod.rs`

## P1 — Fiabilité

### Mode auto sans reprise
- ✅ Une fois rétrogradé (Mistral → Ollama → LT), l'état ne remontait **jamais** : une coupure réseau d'une seconde dégradait l'app pour toute la session
- ✅ `AutoState { kind, downgraded_at }` + re-sondage passé `RECOVERY_AFTER` (90 s), avec notification tray « Reprise → … »
- ✅ `kind: Option<…>` au lieu de `LtOnly` par défaut : un déclenchement avant la fin de la détection force la détection au lieu de tomber sur LanguageTool (qui n'était pas démarré → 12 s de retries puis erreur)
- **Fichier** : `ai/mod.rs`

### Résultats périmés qui écrasent l'affichage
- ✅ Compteur de génération : chaque capture invalide la précédente, un résultat lent arrivant après un nouveau déclenchement est ignoré
- **Fichiers** : `ai/mod.rs` (`next_generation`/`emit`), `main.rs`

### Bug d'affichage LanguageTool
- ✅ `call()` renvoyait la chaîne littérale `"Aucune erreur."` comme correction → le frontend la diffait contre l'original et affichait **tout le texte comme supprimé**
- ✅ Renvoie désormais le texte (corrigé ou non) ; le diff produit naturellement « ✓ Aucune faute détectée »
- **Fichier** : `ai/languagetool.rs`

### Freeze de la webview sur texte long
- ✅ La matrice LCS `Int32Array` n'était pas bornée : une reformulation intégrale d'un texte de 5 000 mots demandait ~400 Mo et figeait la popup
- ✅ Plafond `DIFF_BUDGET = 2 000 000` cellules → repli sur affichage simple sans diff
- **Fichier** : `src/main.js`

## P2 — Latence perçue & robustesse

- ✅ **Presse-papier** : l'attente fixe de 160 ms après le Ctrl+C simulé est remplacée par une surveillance du `GetClipboardSequenceNumber` (poll 10 ms, plafond 300 ms) → ~120 ms gagnés à l'ouverture de la popup
- ✅ **Contention presse-papier** : 3 tentatives espacées de 25 ms — un `Clipboard::new()` en échec ponctuel (Office, navigateur) donnait « (aucun texte) »
- ✅ Pause avant le Ctrl+V de remplacement : 250 ms → 120 ms
- ✅ Retries LanguageTool : 8×1500 ms → 14×900 ms (même budget, on repart dès que le serveur répond)
- ✅ Événement `ai-status` : la popup affiche « Démarrage du correcteur local (~10 s)… » au lieu d'un spinner muet
- ✅ `results[activeKey] || '(indisponible)'` (`??` ne rattrapait pas une chaîne vide)
- **Fichiers** : `src-tauri/src/clipboard.rs`, `src-tauri/src/main.rs`, `ai/languagetool.rs`, `src/main.js`, `src-tauri/Cargo.toml` (feature `Win32_System_DataExchange`)

## Résultats

| Métrique | Avant | Après |
|---|---|---|
| RAM au repos (mode `auto`) | ~1229 Mo | **354 Mo** |
| JVM démarrée au lancement | Toujours | Seulement si le repli l'atteint |
| Ollama, usage sporadique | ~20,7 s | **~1,3 s** |
| Requête bloquée | Popup infinie | Timeout 60 s max |
| Travail dans le hook clavier | Appels Tauri | Arithmétique + `send()` |
| Récupération après incident réseau | Jamais (redémarrage) | Automatique après 90 s |

**Build validé** : ✅ `cargo check --release` 0 erreur / 0 warning, `npm run build` OK  
**Déployé** : ✅ `ReformoJuste-Portable/reformojuste.exe` (ancien binaire sauvegardé en `.bak-avant-optim`)  
**Vérifié au lancement** : ✅ aucune JVM démarrée, 354 Mo, arrêt sans processus orphelin  
**Reste à valider par l'usage** : ressenti du double Ctrl+Space sur plusieurs heures (le symptôme du hook retiré par Windows est intermittent par nature)

---

# Phase 8 — Démarrage automatique avec Windows (2026-08-21)

## Choix technique

Clé `Run` de **`HKEY_CURRENT_USER`**, retenue contre trois alternatives :

| Option | Écartée parce que |
|---|---|
| Tâche planifiée | Demande des droits élevés, disproportionné pour une app utilisateur |
| Raccourci dans `shell:startup` | Impose de générer un `.lnk` (COM) là où une valeur registre suffit |
| `tauri-plugin-autostart` | Nouvelle dépendance pour ~60 lignes de logique — contraire à la règle « préférer l'existant » |

`HKCU\Run` ne demande **aucun droit administrateur**, suit le profil utilisateur (cohérent avec une app portable) et se retire d'une seule suppression de valeur.

`winreg` est déclaré en dépendance directe mais était **déjà dans l'arbre** (tiré par `reqwest` en 0.50 et `tauri` en 0.52) : zéro paquet supplémentaire à compiler, contre ~60 lignes d'`unsafe` Win32 évitées sur un chemin qui écrit dans le registre.

## Implémentation

- ✅ Nouveau module `src-tauri/src/autostart.rs` — `is_enabled` / `set` / `sync_path`
- ✅ Entrée de tray **« Démarrer avec Windows »** cochable, séparée de « Quitter » ; la coche reflète l'état réel de la clé à l'ouverture du menu, pas un état supposé
- ✅ Chemin enregistré **entre guillemets** : un dossier portable dans un chemin à espaces cassait sinon au lancement
- ✅ `sync_path()` au démarrage : si le dossier portable a été déplacé, l'entrée est réécrite vers le nouvel emplacement au lieu de rester morte
- ✅ `disable()` idempotent (une valeur déjà absente n'est pas une erreur)
- ✅ `ai::notify()` extrait de `notify_tray()` : le retour du réglage passe par une bulle système **sans** écraser le tooltip du tray, qui reste l'indicateur permanent de l'IA active
- ✅ Rien n'est activé par défaut — réglage strictement opt-in

## Tests

`cargo test --release autostart` — 2 tests, 2 passés :
- `command_line_est_entre_guillemets` — protection des chemins à espaces
- `activation_desactivation_aller_retour` — aller-retour sur la **vraie** clé Run, activation/désactivation idempotentes, état initial restauré en fin de test (vérifié : aucun résidu dans le registre après exécution)

**Build validé** : ✅ `cargo check --release` 0 erreur / 0 warning, `npm run build` OK  
**Vérifié au lancement** : ✅ app démarrée (34 Mo), aucune JVM, aucune entrée Run créée sans action de l'utilisateur, arrêt propre  
**Reste à valider par l'usage** : la bascule elle-même se fait par clic dans le menu du tray — non automatisable en test

---

# Phase 9 — Forme d'adresse préservée dans les corrections (2026-08-23)

## Symptôme

Les corrections basculaient en **vouvoiement alors que le texte d'origine tutoyait**.
Non systématique : déclenché par le degré de familiarité de l'entrée (élisions
« t'inquiète », « t'es dispo », abréviations « dispo », « stp »).

```
ORIGINAL : T'inquiete pas, je m'en occupe. Tu me redis quand t'es dispo.
AVANT    : Ne vous inquiétez pas, je m'en occupe. Vous me redites quand vous êtes disponible.
APRÈS    : T'inquiète pas, je m'en occupe. Tu me redis quand tu es dispo.
```

## Diagnostic — LanguageTool était hors de cause

LanguageTool était le suspect initial. Écarté en interrogeant le serveur bundlé
directement (`bundle/jre/bin/java.exe` + `languagetool-server.jar`, port de test
dédié) plutôt qu'en raisonnant sur son comportement supposé :

| Entrée | Règle déclenchée | Remplacement proposé |
|---|---|---|
| `tu fait` | `ACCORD_R_PERS_VERBE` | `fais` — jamais `faites` |
| `tu a` | `ACCORD_R_PERS_VERBE` | `as` — jamais `avez` |
| `Dis moi` | `IMP_PRON` | `Dis-moi` |

LanguageTool corrige **par offsets** : il remplace des segments désignés par
position, il n'a structurellement aucun moyen de changer une forme d'adresse, et
son jeu de règles français ne contient aucune catégorie de registre.

**Cause réelle** : `build_prompt()` dans `src-tauri/src/ai/mod.rs`, partagé par
`mistral.rs` et `local.rs`. Il demandait un texte « corrigé orthographiquement et
grammaticalement, sans changer le sens » — rien n'y protégeait la forme d'adresse,
et « sans changer le sens » ne la couvre pas : pour un modèle, tu/vous ne change
pas le sens. Il interprétait donc « corriger » comme « normaliser vers le français
standard ».

⚠️ À retenir : en mode `auto` avec une clé Mistral valide, la correction affichée
vient **toujours** de Mistral — LanguageTool n'est atteint que si Mistral *et*
Ollama tombent. L'onglet « Correction » n'est pas un onglet LanguageTool.

## Implémentation

- ✅ Règle de forme d'adresse ajoutée à `build_prompt` — donc appliquée aux deux
  providers IA d'un seul point, sans toucher `mistral.rs` ni `local.rs`
- ✅ Écrite en **deux passes** : la v1 corrigeait bien `correction` mais laissait
  `professional` et `formal` dériver vers le vouvoiement. La v2 nomme
  explicitement ce mode d'échec — les 5 styles portent sur le vocabulaire et la
  syntaxe, jamais sur la forme d'adresse — et donne un exemple de soutenu qui
  tutoie (« Aurais-tu l'obligeance de… »)
- ✅ Couvre les **deux sens** : un texte vouvoyé ne doit pas devenir tutoyé dans
  `simple` / `creative`
- ✅ 8 lignes ajoutées, aucun autre fichier modifié

## Résultats mesurés

4 phrases (3 en tutoiement + 1 contrôle en vouvoiement) × 2 providers, prompt
**extrait du source compilé** pour que le test porte sur le prompt réel :

| Provider | Écarts après règle v1 | Écarts après règle v2 |
|---|---|---|
| Mistral (`mistral-small-latest`) | 4 | **1** |
| Ollama (`gemma3:4b`) | 7 | **6** |

- Champ `correction` — l'objet du bug : **8/8 juste**, sur les deux providers et
  sur les deux exécutions successives
- Mistral : le seul écart restant est un `simple` qui tutoie sur entrée vouvoyée
- Ollama : les 6 écarts restants sont **tous** `professional` / `formal` sur
  entrée tutoyée. `gemma3:4b` ne tient pas la contrainte contre son a priori
  « professionnel ⇒ vouvoiement » — limite de taille de modèle (4B), pas un défaut
  de prompt, le même prompt passe sur `mistral-small`. Sans impact hors repli.

⚠️ Réserves de méthode : le détecteur de registre utilisé pour compter les écarts
rate les apostrophes typographiques (`t'est` avec ’ U+2019), le compte Mistral est
donc peut-être 2 et non 1 ; et 4 phrases × 1 run à `temperature: 0.7`, c'est un
échantillon indicatif, pas une garantie statistique.

## Non retenu

`temperature: 0.7` (`mistral.rs`) n'a **pas** été baissée : elle fiabiliserait le
dernier écart mais aplatirait aussi la reformulation `creative`. Arbitrage laissé
ouvert.

**Build validé** : ✅ `cargo check` 0 erreur / 0 warning, `tauri build` release en
1 min 07, 0 warning
**Portable régénéré** : ✅ `distribute.ps1` — exe identique au build release
(`cmp`), présence de la nouvelle règle vérifiée par recherche de chaîne **dans le
binaire** (les deux versions font exactement la même taille : ne pas se fier à la
taille pour vérifier qu'un déploiement a pris, seulement à la date)
**Reste à valider par l'usage** : test réel au double Ctrl+Space — les 8 essais
ci-dessus passent par les API directement, pas par la popup Tauri
