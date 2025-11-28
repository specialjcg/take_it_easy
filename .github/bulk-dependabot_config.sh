#!/bin/bash

# =================================================================
#                         CONFIGURATION
# =================================================================
# ⚠️ REMPLACEZ LES VALEURS CI-DESSOUS AVANT L'EXÉCUTION ⚠️

# Votre nom d'utilisateur ou le nom de l'organisation GitHub
ORG_NAME="specialjcg" 

# Chemin ABSOLU vers le dossier de configuration .github SOURCE (DOIT EXISTER !)
# Assurez-vous que ce dossier contient /workflows/dependabot-auto-merge.yml
CONFIG_DIR="/home/jcgouleau/github-config/.github" 

# Nom de la nouvelle branche qui sera créée dans chaque dépôt
BRANCH_NAME="feat/dependabot-auto-merge"

# Message du commit
COMMIT_MESSAGE="feat: Ajout configuration Dependabot Auto-Merge"

# Titre de la Pull Request
PR_TITLE="Configuration : Activer la fusion automatique des Dependabot PRs"

# =================================================================
#                         VÉRIFICATIONS PRÉALABLES
# =================================================================

# Vérifier si l'outil 'gh' (GitHub CLI) est disponible
if ! command -v gh &> /dev/null; then
    echo "ERREUR: L'outil 'gh' (GitHub CLI) n'est pas installé. Veuillez l'installer et vous authentifier."
    exit 1
fi

# Vérifier si l'outil 'jq' est disponible
if ! command -v jq &> /dev/null; then
    echo "ERREUR: L'outil 'jq' est nécessaire. Veuillez l'installer (sudo apt install jq)."
    exit 1
fi

# Vérifier si le dossier de configuration source existe
if [ ! -d "$CONFIG_DIR" ]; then
    echo "ERREUR: Le dossier de configuration source n'existe pas ou le chemin est incorrect : $CONFIG_DIR"
    exit 1
fi

# =================================================================
#                         LOGIQUE PRINCIPALE
# =================================================================

WORKING_DIR=$(pwd)

echo "--- DÉBUT DE LA MISE À JOUR EN MASSE ---"
echo "Organisation ciblée: $ORG_NAME"
echo "Source de configuration: $CONFIG_DIR"

# 1. Récupérer la liste des dépôts (exclut les forks)
echo "1. Récupération de la liste des dépôts..."
gh repo list "$ORG_NAME" --limit 100 --json name,isFork --jq '.[] | select(.isFork | not) | .name' > repo_list.txt

# Vérifier si la liste est vide
if [ ! -s repo_list.txt ]; then
    echo "ERREUR: Aucun dépôt trouvé ou la liste est vide."
    rm repo_list.txt
    exit 1
fi

echo "$(wc -l < repo_list.txt) dépôts à traiter."
echo "-------------------------------------"

# 2. Boucle principale de traitement
while IFS= read -r REPO_NAME; do
    FULL_REPO="$ORG_NAME/$REPO_NAME"
    echo "-> Traitement de $FULL_REPO..."

    # Cloner le dépôt dans un dossier temporaire
    gh repo clone "$FULL_REPO"
    
    if [ -d "$REPO_NAME" ]; then
        cd "$REPO_NAME"

        # Créer et basculer vers une nouvelle branche
        git checkout -b "$BRANCH_NAME"
        
        # Copier le dossier de configuration (le dossier .github complet)
        cp -r "$CONFIG_DIR" .
        
        # Ajouter le dossier .github pour le commit
        git add .github
        
        # Vérifier s'il y a des changements à committer
        if git status --porcelain | grep -q '^\(A\|M\)'; then
            
            # Committer et Pousser les changements
            git commit -m "$COMMIT_MESSAGE"
            git push -u origin "$BRANCH_NAME" --force
            
            # Déterminer la branche principale (main ou master)
            BASE_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD | sed 's|^refs/remotes/origin/||')

            # Créer la Pull Request
            echo "   -> Création de la Pull Request..."
            gh pr create \
                --title "$PR_TITLE" \
                --body "Ce changement ajoute le workflow GitHub Actions pour activer la fusion automatique des PRs Dependabot après la réussite des vérifications de CI/CD." \
                --base "$BASE_BRANCH"
            
        else
            echo "   -> Le dossier .github existe déjà et est à jour. Aucune modification commise."
        fi

        # Retourner au dossier de travail initial
        cd "$WORKING_DIR"
        
        # Nettoyage : Supprimer le dossier cloné pour le prochain dépôt
        rm -rf "$REPO_NAME"
    else
        echo "ERREUR lors du clonage de $FULL_REPO. Passage au suivant."
    fi

    echo "-------------------------------------"
done < repo_list.txt

# 3. Nettoyage final
rm repo_list.txt
echo "Script terminé. Toutes les Pull Requests ont été créées sur GitHub."
