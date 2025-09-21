#!/bin/bash
# Script de test complet pour le projet Take It Easy
# Vérifie tous les tests unitaires et d'intégration

set -e

echo "🧪 =========================================="
echo "🧪 Take It Easy - Test Suite Complete"
echo "🧪 =========================================="

# Couleurs pour l'affichage
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Variables de comptage
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

echo -e "${BLUE}📊 Phase 1: Tests unitaires (lib)${NC}"
echo "----------------------------------------"

# Exécuter les tests unitaires avec timeout
if timeout 60 cargo test --lib --no-fail-fast -- --test-threads=1 2>&1 | tee lib_tests.log; then
    LIB_TESTS=$(grep "test result:" lib_tests.log | grep -o '[0-9]* passed' | cut -d' ' -f1)
    LIB_FAILED=$(grep "test result:" lib_tests.log | grep -o '[0-9]* failed' | cut -d' ' -f1 || echo "0")

    if [ "$LIB_FAILED" = "0" ]; then
        echo -e "${GREEN}✅ Tests unitaires: ${LIB_TESTS}/${LIB_TESTS} réussis${NC}"
        PASSED_TESTS=$((PASSED_TESTS + LIB_TESTS))
    else
        echo -e "${RED}❌ Tests unitaires: ${LIB_TESTS} réussis, ${LIB_FAILED} échoués${NC}"
        PASSED_TESTS=$((PASSED_TESTS + LIB_TESTS))
        FAILED_TESTS=$((FAILED_TESTS + LIB_FAILED))
    fi
    TOTAL_TESTS=$((TOTAL_TESTS + LIB_TESTS + LIB_FAILED))
else
    echo -e "${RED}❌ Échec lors de l'exécution des tests unitaires${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}📊 Phase 2: Tests d'intégration${NC}"
echo "----------------------------------------"

# Exécuter les tests d'intégration
if timeout 30 cargo test --test lib_integration_test -- --test-threads=1 2>&1 | tee integration_tests.log; then
    INT_TESTS=$(grep "test result:" integration_tests.log | grep -o '[0-9]* passed' | cut -d' ' -f1)
    INT_FAILED=$(grep "test result:" integration_tests.log | grep -o '[0-9]* failed' | cut -d' ' -f1 || echo "0")

    if [ "$INT_FAILED" = "0" ]; then
        echo -e "${GREEN}✅ Tests d'intégration: ${INT_TESTS}/${INT_TESTS} réussis${NC}"
        PASSED_TESTS=$((PASSED_TESTS + INT_TESTS))
    else
        echo -e "${RED}❌ Tests d'intégration: ${INT_TESTS} réussis, ${INT_FAILED} échoués${NC}"
        PASSED_TESTS=$((PASSED_TESTS + INT_TESTS))
        FAILED_TESTS=$((FAILED_TESTS + INT_FAILED))
    fi
    TOTAL_TESTS=$((TOTAL_TESTS + INT_TESTS + INT_FAILED))
else
    echo -e "${RED}❌ Échec lors de l'exécution des tests d'intégration${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}📊 Phase 3: Vérification du build${NC}"
echo "----------------------------------------"

# Vérifier que le projet compile sans warnings critiques
if cargo build --quiet 2>&1 | grep -E "(error|warning)" > build.log; then
    WARNING_COUNT=$(grep -c "warning" build.log || echo "0")
    ERROR_COUNT=$(grep -c "error" build.log || echo "0")

    if [ "$ERROR_COUNT" = "0" ]; then
        if [ "$WARNING_COUNT" = "0" ]; then
            echo -e "${GREEN}✅ Build: Compilation réussie sans warnings${NC}"
        else
            echo -e "${YELLOW}⚠️  Build: Compilation réussie avec ${WARNING_COUNT} warnings${NC}"
        fi
    else
        echo -e "${RED}❌ Build: ${ERROR_COUNT} erreurs de compilation${NC}"
        cat build.log
        exit 1
    fi
else
    echo -e "${GREEN}✅ Build: Compilation réussie sans warnings${NC}"
fi

echo ""
echo "🧪 =========================================="
echo "🧪 RÉSUMÉ FINAL"
echo "🧪 =========================================="

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}🎉 SUCCÈS: Tous les tests passent !${NC}"
    echo -e "${GREEN}📊 Total: ${PASSED_TESTS}/${TOTAL_TESTS} tests réussis (100%)${NC}"
    echo ""
    echo "✅ Tests unitaires: $LIB_TESTS/$(($LIB_TESTS + $LIB_FAILED))"
    echo "✅ Tests intégration: $INT_TESTS/$(($INT_TESTS + $INT_FAILED))"
    echo ""
    echo -e "${BLUE}💡 Le projet est prêt pour les refactorings futurs !${NC}"
else
    echo -e "${RED}❌ ÉCHEC: Certains tests échouent${NC}"
    echo -e "${RED}📊 Total: ${PASSED_TESTS}/${TOTAL_TESTS} tests réussis ($(($PASSED_TESTS * 100 / $TOTAL_TESTS))%)${NC}"
    echo -e "${RED}🔥 ${FAILED_TESTS} tests échoués${NC}"
    exit 1
fi

# Nettoyer les fichiers de log temporaires
rm -f lib_tests.log integration_tests.log build.log

echo ""
echo -e "${GREEN}🚀 Script de test terminé avec succès !${NC}"