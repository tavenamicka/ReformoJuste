// Tauri 1 globals (injected when withGlobalTauri = true)
const { invoke }    = window.__TAURI__.tauri;
const { listen }    = window.__TAURI__.event;
const { appWindow } = window.__TAURI__.window;

// ── DOM refs ──────────────────────────────────────────────────────────────────
const elLoading        = document.getElementById('loading');
const elError          = document.getElementById('error');
const elTabs           = document.getElementById('tabs');
const elResult         = document.getElementById('result');
const elStatus         = document.getElementById('result-status');
const elText           = document.getElementById('result-text');
const elReformulate    = document.getElementById('btn-reformulate');
const elReformulations = document.getElementById('reformulations');
const elCopy           = document.getElementById('btn-copy');
const elReplace        = document.getElementById('btn-replace');
const elClose          = document.getElementById('btn-close');
const elLoadingLabel   = elLoading.querySelector('span');
const elSpinner        = elLoading.querySelector('.spinner');

const LOADING_DEFAULT = 'Analyse en cours…';

// Onglets statiques : capturés une fois (évite des querySelectorAll répétés).
const TABS          = [...elTabs.querySelectorAll('.tab')];
const TAB_KEYS      = TABS.map(t => t.dataset.key);
const correctionTab = TABS.find(t => t.dataset.key === 'correction');

// ── State ─────────────────────────────────────────────────────────────────────
let results      = null;
let originalText = '';
let activeKey    = 'correction';

// ── Affichage des états ─────────────────────────────────────────────────────────
function showState(state) {
  const onResult = state === 'result' || state === 'partial';
  elLoading.classList.toggle('hidden', state !== 'loading');
  elError.classList.toggle('hidden',   state !== 'error');
  elTabs.classList.toggle('hidden',    !onResult);
  elResult.classList.toggle('hidden',  !onResult);
}

function setError(msg) {
  elError.textContent = msg;
  showState('error');
}

// Message d'attente (ex. « Démarrage du correcteur local… » émis par le backend).
function setLoadingLabel(msg) {
  elLoadingLabel.textContent = msg || LOADING_DEFAULT;
}

// ── Onglets ─────────────────────────────────────────────────────────────────────
// Marque l'onglet actif (classe + ARIA + roving tabindex) et met à jour activeKey.
function markActiveTab(activeBtn) {
  for (const t of TABS) {
    const on = t === activeBtn;
    t.classList.toggle('active', on);
    t.setAttribute('aria-selected', String(on));
    t.tabIndex = on ? 0 : -1;
  }
  activeKey = activeBtn.dataset.key;
}

// Sort tous les onglets de l'état « en chargement ».
function enableAllTabs() {
  for (const t of TABS) { t.classList.remove('pending'); t.disabled = false; }
  elReformulate.disabled = false;
}

// Replie/déplie le panneau des reformulations (divulgation progressive).
function setReformOpen(open) {
  elReformulations.classList.toggle('hidden', !open);
  elReformulate.classList.toggle('open', open);
  elReformulate.setAttribute('aria-expanded', String(open));
  elReformulate.textContent = open ? 'Reformuler ▴' : 'Reformuler ▾';
}

// Affiche la correction immédiatement, les autres onglets restant en chargement.
function showPartial() {
  showState('partial');
  markActiveTab(correctionTab);
  for (const t of TABS) {
    const isCorrection = t === correctionTab;
    t.classList.toggle('pending', !isCorrection);
    t.disabled = !isCorrection;
  }
  elReformulate.disabled = true;   // reformulations pas encore prêtes
}

// Tous les résultats sont arrivés.
function showFull() {
  showState('result');
  enableAllTabs();
}

// ── Diff & rendu ────────────────────────────────────────────────────────────────
function escapeHtml(s) {
  return s.replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
}

// Plafond de cellules pour la matrice LCS. Au-delà, la mémoire (4 octets par
// cellule) et le temps de calcul figent la webview : une reformulation intégrale
// d'un long texte n'a de toute façon pas de diff lisible à montrer.
const DIFF_BUDGET = 2_000_000;

// Diff mot à mot → { html, changes }. Le texte conservé reste neutre ; un groupe
// contigu de mots modifiés compte pour 1 correction. Les préfixes/suffixes communs
// sont retirés avant le LCS O(n·m) : une correction ne change en général que
// quelques mots, donc la zone centrale réellement analysée reste petite.
// `changes === null` = diff non calculé (texte trop long).
function diffHtml(oldStr, newStr) {
  const a = oldStr.match(/\s+|\S+/g) || [];
  const b = newStr.match(/\s+|\S+/g) || [];

  let lo = 0;
  while (lo < a.length && lo < b.length && a[lo] === b[lo]) lo++;
  let hiA = a.length, hiB = b.length;
  while (hiA > lo && hiB > lo && a[hiA - 1] === b[hiB - 1]) { hiA--; hiB--; }

  const midA = a.slice(lo, hiA);
  const midB = b.slice(lo, hiB);
  const n = midA.length, m = midB.length;

  if (n * m > DIFF_BUDGET) return { html: escapeHtml(newStr), changes: null };

  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      dp[i][j] = midA[i] === midB[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);

  let out = '', changes = 0, inChange = false;
  const keep = t => { out += escapeHtml(t); if (t.trim()) inChange = false; };
  const mark = (t, tag) => {
    if (!t.trim()) { out += escapeHtml(t); return; }
    if (!inChange) changes++;
    inChange = true;
    out += `<${tag}>${escapeHtml(t)}</${tag}>`;
  };

  let i = 0, j = 0;
  while (i < n && j < m) {
    if (midA[i] === midB[j])               { keep(midB[j]); i++; j++; }
    else if (dp[i + 1][j] >= dp[i][j + 1]) { mark(midA[i], 'del'); i++; }
    else                                   { mark(midB[j], 'ins'); j++; }
  }
  while (i < n) { mark(midA[i], 'del'); i++; }
  while (j < m) { mark(midB[j], 'ins'); j++; }

  const prefix = a.slice(0, lo).map(escapeHtml).join('');
  const suffix = a.slice(hiA).map(escapeHtml).join('');
  return { html: prefix + out + suffix, changes };
}

function setStatus(changes) {
  if (changes === null) {           // texte trop long : pas de diff calculé
    elStatus.classList.add('hidden');
    return;
  }
  elStatus.classList.remove('hidden');
  elStatus.classList.toggle('clean', changes === 0);
  elStatus.textContent = changes === 0
    ? '✓ Aucune faute détectée'
    : `${changes} correction${changes > 1 ? 's' : ''}`;
}

function renderResult() {
  if (!results) return;
  // `||` et non `??` : un champ renvoyé vide par le modèle doit aussi basculer
  // sur le libellé de repli.
  const val = results[activeKey] || '(indisponible)';
  // Le diff n'a de sens que pour la correction ; les reformulations sont des réécritures.
  if (activeKey === 'correction' && originalText) {
    const { html, changes } = diffHtml(originalText, val);
    elText.innerHTML = html;   // entrées échappées via escapeHtml → pas d'injection
    setStatus(changes);
  } else {
    elText.textContent = val;
    elStatus.classList.add('hidden');
  }
}

// ── Boutons d'action ────────────────────────────────────────────────────────────
function resetCopyBtn() {
  elCopy.textContent = 'Copier';
  elCopy.classList.remove('success');
}

function resetReplaceBtn() {
  elReplace.textContent = 'Remplacer';
  elReplace.classList.remove('pending');
  elReplace.disabled = false;
}

// ── Interactions onglets (délégation : un seul listener) ─────────────────────────
elTabs.addEventListener('click', e => {
  const btn = e.target.closest('.tab');
  if (!btn || btn.disabled) return;
  if (elReformulations.contains(btn)) setReformOpen(true); // garde le panneau ouvert
  markActiveTab(btn);
  resetCopyBtn();
  renderResult();
});

elReformulate.addEventListener('click', () => {
  if (elReformulate.disabled) return;
  setReformOpen(elReformulations.classList.contains('hidden'));
});

// Navigation clavier du tablist (pattern ARIA) : ←/→ entre onglets visibles, Home/End aux extrêmes.
elTabs.addEventListener('keydown', e => {
  if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) return;
  const tabs = TABS.filter(t => !t.disabled && t.offsetParent !== null);
  if (!tabs.length) return;
  const cur = Math.max(0, tabs.indexOf(document.activeElement));
  const next = e.key === 'Home' ? 0
             : e.key === 'End'  ? tabs.length - 1
             : (cur + (e.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
  e.preventDefault();
  tabs[next].focus();
  tabs[next].click();   // activation automatique
});

// ── Fenêtre ─────────────────────────────────────────────────────────────────────
// Le drag est géré en déclaratif via data-tauri-drag-region sur le <header> (HTML).
elClose.addEventListener('click', () => appWindow.hide());

elReplace.addEventListener('click', async () => {
  if (!results) return;
  elReplace.textContent = '…';
  elReplace.classList.add('pending');
  elReplace.disabled = true;
  try {
    await invoke('replace_text', { text: results[activeKey] });
    resetReplaceBtn();
    appWindow.hide();   // clôt le geste : l'utilisateur repart dans son app
  } catch (e) {
    resetReplaceBtn();
    setError(`Impossible de remplacer : ${e}`);
  }
});

elCopy.addEventListener('click', async () => {
  if (!results) return;
  try {
    await invoke('copy_to_clipboard', { text: results[activeKey] });
    elCopy.textContent = '✓ Copié !';
    elCopy.classList.add('success');
    setTimeout(resetCopyBtn, 1800);
  } catch (e) {
    setError(`Impossible de copier : ${e}`);
  }
});

// ── Raccourcis globaux ──────────────────────────────────────────────────────────
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') { appWindow.hide(); return; }

  const num = Number(e.key);
  if (e.ctrlKey && num >= 1 && num <= TAB_KEYS.length) {
    TABS[num - 1].click();
    return;
  }

  // Entrée = Remplacer, sauf si le focus est sur un bouton (évite le double déclenchement).
  if (e.key === 'Enter' && !e.target.closest('button')
      && results && !elResult.classList.contains('hidden')) {
    e.preventDefault();
    elReplace.click();
  }
});

// ── Événements backend ──────────────────────────────────────────────────────────

// Étape 1 — texte capturé, lancement du traitement.
listen('text-ready', ({ payload }) => {
  const text = payload.text?.trim();

  results      = null;
  originalText = text || '';
  markActiveTab(correctionTab);
  enableAllTabs();
  setReformOpen(false);   // on repart sur la correction seule
  resetCopyBtn();
  resetReplaceBtn();

  if (!text) {
    setError('Aucun texte sélectionné.\nSélectionnez du texte dans une application puis appuyez deux fois sur Ctrl+Space.');
    return;
  }

  setLoadingLabel(LOADING_DEFAULT);
  elSpinner.style.display = '';
  showState('loading');
  // Fire-and-forget — les résultats arrivent via ai-partial / ai-result.
  invoke('process_text', { text }).catch(e => setError(`Erreur : ${e}`));
});

// Attente prolongée annoncée par le backend (ex. démarrage de LanguageTool).
listen('ai-status', ({ payload }) => setLoadingLabel(payload));

// Étape 2 (mode hybride) — LanguageTool terminé, on montre la correction tout de suite.
listen('ai-partial', ({ payload }) => {
  if (!results) results = {};
  results.correction = payload.correction;
  showPartial();
  renderResult();
});

// Étape 3 — tous les résultats sont prêts.
listen('ai-result', ({ payload }) => {
  results = payload;
  showFull();
  renderResult();   // conserve l'onglet actif (ex. correction déjà affichée en partial)
});

listen('ai-error', ({ payload }) => {
  setError(`Erreur IA :\n${payload}`);
});

// ── État initial ────────────────────────────────────────────────────────────────
showState('loading');
setLoadingLabel('Prêt — double Ctrl+Space pour analyser');
elSpinner.style.display = 'none';
