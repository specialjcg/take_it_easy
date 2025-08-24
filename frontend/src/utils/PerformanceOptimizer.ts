// utils/PerformanceOptimizer.ts - Solutions optimisées pour les problèmes UX identifiés
import { batch, createMemo, createSignal, Accessor, Setter } from 'solid-js';

/**
 * 🚀 CLASSE PRINCIPALE - Optimisations de performance critique
 */
export class PerformanceOptimizer {
    
    /**
     * 🎯 SOLUTION #1: Circuit Breaker pour requêtes réseau
     * Évite les cascades de requêtes et protège contre les pics de charge
     */
    static createCircuitBreaker<T>(
        requestFn: () => Promise<T>,
        threshold: number = 5,
        timeout: number = 30000
    ) {
        let state: 'CLOSED' | 'OPEN' | 'HALF_OPEN' = 'CLOSED';
        let failureCount = 0;
        let lastFailureTime = 0;
        let successCount = 0;
        
        return async (): Promise<T> => {
            const now = Date.now();
            
            // État OPEN: vérifier si on peut passer en HALF_OPEN
            if (state === 'OPEN') {
                if (now - lastFailureTime < timeout) {
                    throw new Error('Circuit breaker is OPEN - service temporarily unavailable');
                }
                state = 'HALF_OPEN';
                successCount = 0;
            }
            
            try {
                const result = await requestFn();
                
                // Succès : réinitialiser ou confirmer fermeture
                if (state === 'HALF_OPEN') {
                    successCount++;
                    if (successCount >= 2) { // 2 succès pour confirmer
                        state = 'CLOSED';
                        failureCount = 0;
                    }
                } else if (state === 'CLOSED') {
                    failureCount = Math.max(0, failureCount - 1); // Récupération progressive
                }
                
                return result;
            } catch (error) {
                failureCount++;
                lastFailureTime = now;
                
                // État CLOSED → OPEN si seuil atteint
                if (state === 'CLOSED' && failureCount >= threshold) {
                    state = 'OPEN';
                } else if (state === 'HALF_OPEN') {
                    state = 'OPEN'; // Retour immédiat en OPEN si échec en HALF_OPEN
                }
                
                throw error;
            }
        };
    }
    
    /**
     * 🎨 SOLUTION #2: Optimiseur de Canvas avec RequestAnimationFrame
     * Évite les redraws inutiles et optimise les performances graphiques
     */
    static createCanvasOptimizer() {
        let rafId: number | undefined;
        let isDrawPending = false;
        let lastDrawnHash = '';
        
        return {
            scheduleRedraw: (
                canvas: HTMLCanvasElement,
                drawFn: (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement) => Promise<void> | void,
                stateHash: string
            ) => {
                // Skip si même état ou dessin en cours
                if (isDrawPending || stateHash === lastDrawnHash) {
                    return Promise.resolve();
                }
                
                return new Promise<void>((resolve, reject) => {
                    // Cleanup précédent
                    if (rafId) {
                        cancelAnimationFrame(rafId);
                    }
                    
                    isDrawPending = true;
                    rafId = requestAnimationFrame(async () => {
                        try {
                            const ctx = canvas.getContext('2d');
                            if (!ctx) {
                                throw new Error('Cannot get 2D context');
                            }
                            
                            await drawFn(ctx, canvas);
                            lastDrawnHash = stateHash;
                            resolve();
                        } catch (error) {
                            reject(error);
                        } finally {
                            isDrawPending = false;
                            rafId = undefined;
                        }
                    });
                });
            },
            
            cleanup: () => {
                if (rafId) {
                    cancelAnimationFrame(rafId);
                    rafId = undefined;
                }
                isDrawPending = false;
                lastDrawnHash = '';
            },
            
            isDrawing: () => isDrawPending
        };
    }
    
    /**
     * 📦 SOLUTION #3: Cache LRU intelligent pour images
     * Évite les fuites mémoire et optimise le chargement d'images
     */
    static createImageCache(maxSize: number = 100) {
        const cache = new Map<string, HTMLImageElement>();
        const loadingPromises = new Map<string, Promise<HTMLImageElement>>();
        
        const removeOldest = () => {
            if (cache.size >= maxSize) {
                const firstKey = cache.keys().next().value;
                if (firstKey) {
                    const img = cache.get(firstKey);
                    img?.remove?.(); // Cleanup DOM si possible
                    cache.delete(firstKey);
                }
            }
        };
        
        return {
            async preload(src: string): Promise<HTMLImageElement> {
                // Déjà en cache
                if (cache.has(src)) {
                    const img = cache.get(src)!;
                    // Move to end (LRU)
                    cache.delete(src);
                    cache.set(src, img);
                    return img;
                }
                
                // Déjà en cours de chargement
                if (loadingPromises.has(src)) {
                    return loadingPromises.get(src)!;
                }
                
                // Nouveau chargement
                const loadPromise = new Promise<HTMLImageElement>((resolve, reject) => {
                    const img = new Image();
                    
                    img.onload = () => {
                        removeOldest();
                        cache.set(src, img);
                        loadingPromises.delete(src);
                        resolve(img);
                    };
                    
                    img.onerror = () => {
                        loadingPromises.delete(src);
                        // Créer une image placeholder transparente
                        const emptyImg = new Image();
                        emptyImg.width = 1;
                        emptyImg.height = 1;
                        resolve(emptyImg);
                    };
                    
                    img.src = src;
                });
                
                loadingPromises.set(src, loadPromise);
                return loadPromise;
            },
            
            get(src: string): HTMLImageElement | null {
                const img = cache.get(src);
                if (img) {
                    // Move to end (LRU)
                    cache.delete(src);
                    cache.set(src, img);
                    return img;
                }
                return null;
            },
            
            async preloadBatch(sources: string[]): Promise<HTMLImageElement[]> {
                const validSources = sources.filter(src => src && !src.includes('000'));
                const promises = validSources.map(src => this.preload(src));
                return Promise.allSettled(promises).then(results => 
                    results
                        .filter((result): result is PromiseFulfilledResult<HTMLImageElement> => 
                            result.status === 'fulfilled'
                        )
                        .map(result => result.value)
                );
            },
            
            clear(): void {
                cache.forEach(img => img?.remove?.()); // Cleanup DOM
                cache.clear();
                loadingPromises.clear();
            },
            
            getStats() {
                return {
                    cacheSize: cache.size,
                    loadingCount: loadingPromises.size,
                    maxSize
                };
            }
        };
    }
    
    /**
     * ⚡ SOLUTION #4: Optimiseur de polling intelligent
     * Adapte dynamiquement la fréquence selon l'activité utilisateur
     */
    static createSmartPolling() {
        let interval: number | undefined;
        let consecutiveErrors = 0;
        let lastActivity = 0;
        let isUserActive = true;
        
        // Détection d'activité utilisateur
        const updateActivity = () => {
            lastActivity = Date.now();
            isUserActive = true;
        };
        
        // Event listeners pour détecter l'activité
        if (typeof window !== 'undefined') {
            ['mousemove', 'keydown', 'click', 'scroll', 'touchstart'].forEach(event => {
                window.addEventListener(event, updateActivity, { passive: true });
            });
            
            // Détection d'inactivité
            setInterval(() => {
                const timeSinceActivity = Date.now() - lastActivity;
                isUserActive = timeSinceActivity < 60000; // 1 minute d'inactivité = inactif
            }, 30000);
        }
        
        const getOptimalInterval = (baseInterval: number): number => {
            // Facteur d'erreur (backoff exponential limité)
            const errorFactor = Math.min(Math.pow(1.5, consecutiveErrors), 8);
            
            // Facteur d'activité utilisateur
            const activityFactor = isUserActive ? 1 : 2; // 2x plus lent si inactif
            
            // Facteur de visibilité de la page
            const visibilityFactor = (typeof document !== 'undefined' && document.hidden) ? 3 : 1;
            
            return Math.round(baseInterval * errorFactor * activityFactor * visibilityFactor);
        };
        
        return {
            start: (
                pollingFn: () => Promise<any>,
                baseInterval: number = 3000
            ) => {
                const poll = async () => {
                    try {
                        await pollingFn();
                        consecutiveErrors = Math.max(0, consecutiveErrors - 1); // Récupération progressive
                    } catch (error) {
                        consecutiveErrors++;
                        console.warn(`Polling error (${consecutiveErrors} consecutive):`, error);
                    }
                    
                    const nextInterval = getOptimalInterval(baseInterval);
                    interval = window.setTimeout(poll, nextInterval);
                };
                
                // Démarrage immédiat
                poll();
            },
            
            stop: () => {
                if (interval) {
                    clearTimeout(interval);
                    interval = undefined;
                }
            },
            
            markError: () => {
                consecutiveErrors++;
            },
            
            markSuccess: () => {
                consecutiveErrors = Math.max(0, consecutiveErrors - 1);
            },
            
            getStats: () => ({
                consecutiveErrors,
                isUserActive,
                timeSinceActivity: Date.now() - lastActivity
            })
        };
    }
    
    /**
     * 🎭 SOLUTION #5: Gestionnaire d'état de chargement centralisé
     * Évite les états loading incohérents et les flashs
     */
    static createLoadingManager() {
        const [loadingStates, setLoadingStates] = createSignal<Record<string, boolean>>({});
        
        const isLoading = createMemo(() => {
            const states = loadingStates();
            return Object.values(states).some(Boolean);
        });
        
        const setLoading = (key: string, loading: boolean) => {
            batch(() => {
                setLoadingStates(prev => ({
                    ...prev,
                    [key]: loading
                }));
            });
        };
        
        const withLoading = async <T>(
            key: string,
            asyncFn: () => Promise<T>,
            onError?: (error: any) => void
        ): Promise<T | null> => {
            setLoading(key, true);
            try {
                const result = await asyncFn();
                return result;
            } catch (error) {
                onError?.(error);
                return null;
            } finally {
                setLoading(key, false);
            }
        };
        
        return {
            isLoading,
            isLoadingSpecific: (key: string) => createMemo(() => loadingStates()[key] || false),
            setLoading,
            withLoading,
            clearAll: () => setLoadingStates({}),
            getActiveKeys: () => Object.keys(loadingStates()).filter(key => loadingStates()[key])
        };
    }
    
    /**
     * 🚀 SOLUTION #6: Débouncer intelligent pour actions utilisateur
     * Évite les actions multiples et améliore la réactivité perçue
     */
    static createActionDebouncer() {
        const pending = new Map<string, number>();
        const lastExecution = new Map<string, number>();
        
        return {
            debounce: <T extends any[], R>(
                key: string,
                fn: (...args: T) => Promise<R> | R,
                delay: number = 300,
                immediate: boolean = false
            ) => {
                return async (...args: T): Promise<R | void> => {
                    const now = Date.now();
                    const lastTime = lastExecution.get(key) || 0;
                    
                    // Protection anti double-click immédiate
                    if (now - lastTime < 100) {
                        return;
                    }
                    
                    // Cleanup précédent timeout
                    const existingTimeout = pending.get(key);
                    if (existingTimeout) {
                        clearTimeout(existingTimeout);
                    }
                    
                    // Exécution immédiate si demandée et pas d'exécution récente
                    if (immediate && (now - lastTime > delay)) {
                        lastExecution.set(key, now);
                        pending.delete(key);
                        return await fn(...args);
                    }
                    
                    // Exécution retardée
                    return new Promise<R>((resolve, reject) => {
                        const timeoutId = window.setTimeout(async () => {
                            try {
                                lastExecution.set(key, Date.now());
                                pending.delete(key);
                                const result = await fn(...args);
                                resolve(result);
                            } catch (error) {
                                reject(error);
                            }
                        }, delay);
                        
                        pending.set(key, timeoutId);
                    });
                };
            },
            
            cancel: (key: string) => {
                const timeoutId = pending.get(key);
                if (timeoutId) {
                    clearTimeout(timeoutId);
                    pending.delete(key);
                }
            },
            
            cancelAll: () => {
                pending.forEach(timeoutId => clearTimeout(timeoutId));
                pending.clear();
            },
            
            isPending: (key: string) => pending.has(key)
        };
    }
}

/**
 * 🎯 HOOKS OPTIMISÉS PRÊTS À L'USAGE
 */

/**
 * Hook pour Canvas optimisé
 */
export const useOptimizedCanvas = () => {
    const optimizer = PerformanceOptimizer.createCanvasOptimizer();
    
    // Cleanup automatique
    if (typeof window !== 'undefined') {
        window.addEventListener('beforeunload', () => optimizer.cleanup());
    }
    
    return optimizer;
};

/**
 * Hook pour Cache d'images optimisé  
 */
export const useOptimizedImageCache = (maxSize: number = 100) => {
    const cache = PerformanceOptimizer.createImageCache(maxSize);
    
    // Cleanup automatique
    if (typeof window !== 'undefined') {
        window.addEventListener('beforeunload', () => cache.clear());
    }
    
    return cache;
};

/**
 * Hook pour Polling intelligent
 */
export const useSmartPolling = () => {
    const polling = PerformanceOptimizer.createSmartPolling();
    
    // Cleanup automatique
    if (typeof window !== 'undefined') {
        window.addEventListener('beforeunload', () => polling.stop());
    }
    
    return polling;
};

/**
 * Hook pour Loading state centralisé
 */
export const useLoadingManager = () => {
    return PerformanceOptimizer.createLoadingManager();
};

/**
 * Hook pour Actions débouncées
 */
export const useActionDebouncer = () => {
    const debouncer = PerformanceOptimizer.createActionDebouncer();
    
    // Cleanup automatique
    if (typeof window !== 'undefined') {
        window.addEventListener('beforeunload', () => debouncer.cancelAll());
    }
    
    return debouncer;
};