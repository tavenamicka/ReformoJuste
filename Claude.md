# Claude Configuration

## Skill Généraliste Activé

Ce projet utilise le **SKILL_generaliste** pour personnaliser l'exécution des tâches.

### 📋 Référence Rapide des Flags

| Flag | Effet | Exemple |
|------|-------|---------|
| `-a` | Mode automatique (sans validation) | `Crée une fonction -a` |
| `-examine` | Auto-revue critique du travail | `Refactorise ce code -examine` |
| `-test` | Création et exécution de tests | `Code une API -test` |
| `-save` | Sauvegarde structurée des résultats | `Génère des données -save` |
| `-economy` | Optimisation des tokens | `Analyse ce fichier -economy` |
| `-resume` | Reprend une tâche interrompue | `Continue -resume` |

### 🎯 Utilisation

Incluez simplement un ou plusieurs flags dans votre demande :

```
Crée une API FastAPI -a -test -save
Analyse ce dataset -examine -economy
Refactorise le module -test -save
```

### 📚 Documentation Complète

Pour plus de détails, consultez : `SKILL_generaliste.md`

### 🚀 Combinaisons Recommandées

- **Développement rapide** : `-a`
- **Code critique** : `-examine -test`
- **Production** : `-a -examine -test -save`
- **Analyse légère** : `-economy -a`
- **Tâches longues** : `-save -resume`

### ✨ Notes

- Les flags sont reconnus **automatiquement**
- Aucune configuration supplémentaire requise
- Compatible avec tous les types de projets
- Utilisable dans Claude Code, Cowork, ou chat normal

---

**Version** : 1.0  
**Créé** : 2026-05-27  
**Status** : ✅ Actif
