# Skill : Execution Control (Contrôle d'Exécution)

## Vue d'ensemble

Ce skill généraliste permet de **personnaliser le mode d'exécution des tâches** via des flags de contrôle qui modifient le comportement de l'IA lors du traitement de demandes. Il est utile pour tous les types de tâches : développement, création de contenu, analyse, génération de code, etc.

## Déclencheurs

Ce skill s'active quand l'utilisateur inclut **au moins un** des flags suivants dans sa demande :

- `-a` ou `--auto` : Mode automatique
- `-examine` ou `--review` : Auto-revue obligatoire
- `-test` ou `--tests` : Création et exécution de tests
- `-save` ou `--output` : Sauvegarde des résultats
- `-economy` ou `--tokens` : Optimisation des tokens
- `-resume` ou `--continue` : Reprise de tâche

**Exemple de déclenchement :**
```
Crée une fonction Python -a -test -save
Analyse ce fichier -examine -economy
Reprends le script -resume
```

## Comportements par flag

### 1. `-a` / `--auto` (Mode Automatique)

**Effet :** L'IA procède sans demander de validation intermédiaire.

- ✅ Exécute les décisions de manière autonome
- ✅ Ne pause pas pour confirmation utilisateur
- ✅ Enchaîne les étapes naturellement
- ❌ Ne demande pas de clarification (utilise le contexte donné)

**Exemple d'utilisation :**
```
Refactorise ce code JavaScript -a
→ L'IA refactorise directement sans demander "voulez-vous garder X fonctionnalité ?"
```

**Contexte appliqué :** Idéal pour les tâches bien définies où l'IA a assez d'informations.

---

### 2. `-examine` / `--review` (Auto-Revue)

**Effet :** L'IA effectue une **revue critique de son propre travail** avant de le présenter.

- ✅ Identifie les erreurs potentielles
- ✅ Vérifie la cohérence et la complétude
- ✅ Propose des améliorations
- ✅ Signale les points faibles
- ✅ Re-formule si nécessaire

**Exemple d'utilisation :**
```
Génère une requête SQL complexe -examine
→ L'IA crée la requête, la teste mentalement, identifie les cas limites, puis la revise
```

**Processus interne :**
1. Génère le contenu/code/analyse
2. Revue critique : "Qu'est-ce qui pourrait mal tourner ?"
3. Amélioration et corrections
4. Présentation avec explications des changements

**Contexte appliqué :** Pour les tâches critiques, les codes en production, les analyses importantes.

---

### 3. `-test` / `--tests` (Tests Explicites)

**Effet :** L'IA **crée et exécute automatiquement des tests** pour valider le travail.

- ✅ Génère des cas de test (unitaires, intégration, edge cases)
- ✅ Exécute les tests si possible
- ✅ Reporte les résultats
- ✅ Corrige les bugs détectés
- ✅ Documente la couverture de test

**Exemple d'utilisation :**
```
Crée une fonction de validation -test
→ L'IA crée la fonction + tests unitaires + exécution + rapport de couverture
```

**Types de tests selon le contexte :**
- **Code :** Tests unitaires, intégration, cas limites
- **Requêtes :** Tests SQL avec données sample
- **Workflows :** Tests de chaînes d'exécution
- **Documents :** Vérification de cohérence et complétude
- **API :** Tests de réponse et validation de schéma

**Contexte appliqué :** Tâches de développement, code critique, APIs.

---

### 4. `-save` / `--output` (Sauvegarde Structurée)

**Effet :** Tous les **outputs générés sont sauvegardés** dans un dossier organisé.

- ✅ Crée un dossier `outputs_YYYYMMDD_HHMMSS/`
- ✅ Organise les fichiers par type (code, docs, assets, tests)
- ✅ Génère un index/manifest des fichiers
- ✅ Préserve la structure du projet
- ✅ Crée des logs d'exécution

**Structure générée :**
```
outputs_20260527_143022/
├── code/
│   ├── main.py
│   ├── utils.py
│   └── tests.py
├── docs/
│   ├── README.md
│   └── API.md
├── assets/
│   ├── images/
│   └── data/
├── logs/
│   ├── execution.log
│   └── manifest.json
└── manifest.txt
```

**Contenu du manifest :**
```json
{
  "task": "Description de la tâche",
  "timestamp": "2026-05-27T14:30:22Z",
  "flags": ["-save", "-test"],
  "files_generated": [
    {"path": "code/main.py", "size": "2.3KB", "type": "python"},
    ...
  ],
  "execution_status": "success",
  "notes": "..."
}
```

**Contexte appliqué :** Tous les types de projets, archivage, traçabilité.

---

### 5. `-economy` / `--tokens` (Optimisation des Tokens)

**Effet :** L'IA **optimise sa réponse pour minimiser les tokens** utilisés.

- ✅ Élimine la verbosité
- ✅ Utilise des formats compacts (JSON, YAML, tables)
- ✅ Abrège les explications sans perdre l'essence
- ✅ Préfère le code aux descriptions
- ✅ Regroupe les informations connexes

**Modifications de style :**
```
❌ SANS -economy :
"Voici une fonction Python qui valide les emails. 
Elle utilise une expression régulière pour..."

✅ AVEC -economy :
```python
import re
def validate_email(email):
    return re.match(r'^[^@]+@[^@]+\.[^@]+$', email)
```
Notes: Regex basique, à adapter selon vos règles métier.
```

**Cas d'usage spécifiques :**
- Longs documents → résumés structurés
- Code commenté → code concis + docstrings
- Explications longues → bullets points avec liens
- Listes verbales → tableaux/JSON

**Contexte appliqué :** Tâches répétitives, contextes longs, limitations de tokens.

---

### 6. `-resume` / `--continue` (Reprise Automatique)

**Effet :** L'IA **reprend automatiquement une tâche incomplète** à partir de son état antérieur.

- ✅ Récupère le contexte précédent
- ✅ Identifie où l'exécution s'est arrêtée
- ✅ Continue sans redémarrer
- ✅ Préserve la cohérence
- ✅ Log les reprises

**Processus de reprise :**
1. Cherche les marqueurs de progression (fichiers, logs, états partiels)
2. Charge l'état précédent (variables, fichiers générés, étapes complétées)
3. Identifie le point d'arrêt
4. Continue l'exécution de manière fluide
5. Remerge les résultats partiels

**Exemple d'utilisation :**
```
Message 1: "Génère 100 lignes de données -save"
→ (Génère 50 lignes, puis timeout ou interruption)

Message 2: "Continue -resume -save"
→ L'IA reprend à la ligne 51 et complète jusqu'à 100
```

**Marqueurs de reprise :**
- Fichiers partiellement générés
- Logs d'exécution avec checkpoints
- États intermédiaires en JSON
- Commentaires de progression dans le code

**Contexte appliqué :** Tâches longues, traitements par lot, pipelines complexes.

---

## Combinaisons de Flags Recommandées

| Combinaison | Cas d'usage | Description |
|---|---|---|
| `-a` + `-save` | Batch processing | Automatise et archive |
| `-examine` + `-test` | Code critique | Revue + validation complète |
| `-test` + `-save` | Développement | Tests + archivage |
| `-economy` + `-a` | Tâches répétitives | Rapide et léger |
| `-resume` + `-save` | Tâches longues | Reprise sûre et tracée |
| `-a` + `-examine` + `-test` + `-save` | Production | Mode complet robuste |

---

## Contraintes et Limitations

| Flag | Limitation |
|---|---|
| `-a` | Peut exécuter sans assez de contexte → utiliser `-examine` en cas de doute |
| `-examine` | Augmente la taille de la réponse → combiner avec `-economy` si nécessaire |
| `-test` | Non applicable aux tâches sans critères mesurables (créativité pure) |
| `-save` | Nécessite une structure claire des fichiers |
| `-economy` | Peut sacrifier la clarté pédagogique → ne pas utiliser pour apprentissage |
| `-resume` | Requiert que l'état antérieur soit conservé et identifiable |

---

## Priorité des Flags

Quand plusieurs flags sont en conflit, cet ordre de priorité s'applique :

1. **`-examine`** (revue prévient les erreurs)
2. **`-test`** (validation est critique)
3. **`-save`** (traçabilité)
4. **`-resume`** (continuité)
5. **`-economy`** (optimisation)
6. **`-a`** (automatisation)

*Exemple :* `-a -examine` → L'IA auto-examine avant d'exécuter automatiquement, au lieu de d'exécuter sans vérification.

---

## Format de Réponse avec Flags Actifs

L'IA structure sa réponse selon les flags :

### `-a` Seul
```
[Exécution directe du travail]
Résultat : ...
```

### `-examine` Activé
```
[Travail généré]

## 🔍 Auto-Revue
- ✅ Point fort 1
- ⚠️ Amélioration 1
- 🔧 Correction appliquée

[Travail revu]
```

### `-test` Activé
```
[Travail généré]

## ✅ Tests
- Test 1 : PASS
- Test 2 : FAIL → Correction
- Test 3 : PASS

Couverture : 95%
```

### `-save` Activé
```
[Travail généré]

## 💾 Sauvegarde
Dossier créé : outputs_20260527_143022/
Fichiers :
- code/main.py (2.3KB)
- docs/README.md (1.1KB)
...
```

### `-economy` Activé
```
[Version compacte du travail]
Tokens économisés : ~40%
```

### `-resume` Activé
```
## 📋 Reprise
État antérieur chargé : 45% complété
Continuant à partir de : Étape 3/7

[Travail complété]
```

---

## Exemples Complets

### Exemple 1 : Développement robuste
```
Crée une API FastAPI avec authentification JWT -a -examine -test -save
```
**Résultat :**
1. Code généré directement (auto)
2. Revue critique automatique (examine)
3. Tests unitaires + exécution (test)
4. Tous les fichiers archivés (save)

### Exemple 2 : Analyse rapide et légère
```
Analyse ce dataset CSV -economy -a
```
**Résultat :**
1. Réponse compacte sans verbosité (economy)
2. Résultats directs sans demandes intermédiaires (auto)

### Exemple 3 : Tâche longue interruptible
```
Message 1: Génère 500 lignes de test -save
Message 2 (après interruption): Continue -resume
```
**Résultat :**
1. Première génération sauvegardée (save)
2. Reprise fluide au point d'arrêt (resume)

### Exemple 4 : Code pour production
```
Refactorise ce module -examine -test -save -economy
```
**Résultat :**
1. Code revu pour les erreurs (examine)
2. Tests complets (test)
3. Archivé pour traçabilité (save)
4. Format compact (economy)

---

## Notes d'Implémentation

- **Logs de contexte :** Utilisez des marqueurs `[FLAG: -x]` pour tracer l'exécution
- **Détection automatique :** Reconnaître les flags au début ou à la fin de la demande
- **Stacking :** Tous les flags peuvent être cumulés sans conflit
- **Fallback :** Si une tâche n'est pas compatible avec un flag, l'ignorer avec une note
- **Transparence :** Toujours signaler les flags détectés dans la réponse

---

## Checklist d'Utilisation

- [ ] Ai-je bien identifié le type de tâche ?
- [ ] Quels sont mes objectifs ? (vitesse / qualité / traçabilité)
- [ ] Quels flags correspondent à mes besoins ?
- [ ] Y a-t-il des conflits entre mes flags ?
- [ ] Dois-je économiser des tokens ?
- [ ] La tâche est-elle interruptible / à reprendre ?
- [ ] Ai-je besoin de traces d'exécution ?

---

## Versions et Historique

| Version | Date | Changements |
|---|---|---|
| 1.0 | 2026-05-27 | Version initiale - 6 flags fondamentaux |
| 1.1 | - | Planifié : Flags composés (`-quick`, `-thorough`) |
| 1.2 | - | Planifié : Intégration avec gestionnaires d'état persistant |

---

**Créé pour :** Mickatch  
**Usage :** Généraliste, tous types de tâches  
**Langue :** Français + exemples bilingues  
**Dépendances :** Aucune (skill pur, basé sur instructions)
