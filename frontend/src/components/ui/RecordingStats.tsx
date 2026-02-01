// RecordingStats.tsx - Display game recording statistics
import { Component, createSignal, onMount, onCleanup, Show } from 'solid-js';

interface RecordingStatsData {
    enabled: boolean;
    active_games: number;
    recording_dir: string;
    today_file: string | null;
}

export const RecordingStats: Component = () => {
    const [stats, setStats] = createSignal<RecordingStatsData | null>(null);
    const [error, setError] = createSignal<string | null>(null);
    const [loading, setLoading] = createSignal(true);

    const fetchStats = async () => {
        try {
            const response = await fetch('http://localhost:51051/api/recording-stats');
            if (!response.ok) {
                throw new Error(`HTTP error: ${response.status}`);
            }
            const data: RecordingStatsData = await response.json();
            setStats(data);
            setError(null);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to fetch recording stats');
            setStats(null);
        } finally {
            setLoading(false);
        }
    };

    let intervalId: number;
    onMount(() => {
        fetchStats();
        intervalId = window.setInterval(fetchStats, 30000);
    });

    onCleanup(() => {
        if (intervalId) {
            window.clearInterval(intervalId);
        }
    });

    return (
        <div class="recording-stats">
            <div class="recording-stats-header">
                <span class="recording-icon">
                    {stats()?.enabled ? '🔴' : '⚪'}
                </span>
                <span class="recording-title">Recording</span>
            </div>

            <Show when={loading()}>
                <div class="recording-loading">Loading...</div>
            </Show>

            <Show when={error()}>
                <div class="recording-error">
                    <span>⚠️</span>
                    <span>{error()}</span>
                </div>
            </Show>

            <Show when={stats()}>
                <div class="recording-details">
                    <span>
                        Status: <strong>{stats()!.enabled ? 'Active' : 'Disabled'}</strong>
                    </span>
                    <Show when={stats()!.enabled}>
                        <span>
                            Active games: <strong>{stats()!.active_games}</strong>
                        </span>
                        <span>
                            Directory: <code>{stats()!.recording_dir}</code>
                        </span>
                    </Show>
                </div>
            </Show>
        </div>
    );
};

export default RecordingStats;
