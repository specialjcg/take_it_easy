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

                    // ✅ DOUBLE REQUESTANIMATIONFRAME POUR STABILITÉ
                    rafId = requestAnimationFrame(() => {
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
                });
            }
