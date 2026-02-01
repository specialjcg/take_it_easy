// mvu/auth.ts - Gestion de l'authentification

// ============================================================================
// TYPES
// ============================================================================

export interface User {
    id: string;
    email: string;
    username: string;
    emailVerified: boolean;
}

export interface AuthState {
    isAuthenticated: boolean;
    user: User | null;
    token: string | null;
}

// ============================================================================
// AUTH MESSAGES
// ============================================================================

export type LoginRequest = { type: 'LOGIN_REQUEST'; email: string; password: string };
export type LoginSuccess = { type: 'LOGIN_SUCCESS'; user: User; token: string };
export type LoginFailure = { type: 'LOGIN_FAILURE'; error: string };

export type RegisterRequest = {
    type: 'REGISTER_REQUEST';
    email: string;
    username: string;
    password: string;
};
export type RegisterSuccess = { type: 'REGISTER_SUCCESS'; user: User; token: string };
export type RegisterFailure = { type: 'REGISTER_FAILURE'; error: string };

export type LogoutRequest = { type: 'LOGOUT_REQUEST' };
export type LogoutComplete = { type: 'LOGOUT_COMPLETE' };

export type CheckAuthRequest = { type: 'CHECK_AUTH_REQUEST' };
export type CheckAuthSuccess = { type: 'CHECK_AUTH_SUCCESS'; user: User };
export type CheckAuthFailure = { type: 'CHECK_AUTH_FAILURE' };

export type AuthMsg =
    | LoginRequest
    | LoginSuccess
    | LoginFailure
    | RegisterRequest
    | RegisterSuccess
    | RegisterFailure
    | LogoutRequest
    | LogoutComplete
    | CheckAuthRequest
    | CheckAuthSuccess
    | CheckAuthFailure;

// ============================================================================
// AUTH MESSAGE CONSTRUCTORS
// ============================================================================

export const authMsg = {
    login: (email: string, password: string): AuthMsg => ({
        type: 'LOGIN_REQUEST',
        email,
        password,
    }),
    register: (email: string, username: string, password: string): AuthMsg => ({
        type: 'REGISTER_REQUEST',
        email,
        username,
        password,
    }),
    logout: (): AuthMsg => ({ type: 'LOGOUT_REQUEST' }),
    checkAuth: (): AuthMsg => ({ type: 'CHECK_AUTH_REQUEST' }),
};

// ============================================================================
// LOCAL STORAGE
// ============================================================================

const TOKEN_KEY = 'auth_token';
const USER_KEY = 'auth_user';

export const saveAuth = (token: string, user: User): void => {
    localStorage.setItem(TOKEN_KEY, token);
    localStorage.setItem(USER_KEY, JSON.stringify(user));
};

export const loadAuth = (): { token: string | null; user: User | null } => {
    const token = localStorage.getItem(TOKEN_KEY);
    const userStr = localStorage.getItem(USER_KEY);
    const user = userStr ? JSON.parse(userStr) : null;
    return { token, user };
};

export const clearAuth = (): void => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
};

export const getToken = (): string | null => {
    return localStorage.getItem(TOKEN_KEY);
};

// ============================================================================
// API CALLS
// ============================================================================

// En développement, utiliser l'URL directe du backend car le proxy Vite a des problèmes
const API_BASE = import.meta.env.DEV ? 'http://localhost:51051/auth' : '/auth';

export const authApi = {
    async login(email: string, password: string): Promise<{ success: boolean; user?: User; token?: string; error?: string }> {
        try {
            const response = await fetch(`${API_BASE}/login`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email, password }),
            });

            if (response.ok) {
                const data = await response.json();
                return { success: true, user: data.user, token: data.token };
            } else {
                const error = await response.json();
                return { success: false, error: error.error || 'Erreur de connexion' };
            }
        } catch (e) {
            return { success: false, error: 'Erreur réseau' };
        }
    },

    async register(email: string, username: string, password: string): Promise<{ success: boolean; user?: User; token?: string; error?: string }> {
        try {
            const response = await fetch(`${API_BASE}/register`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email, username, password }),
            });

            if (response.ok) {
                const data = await response.json();
                return { success: true, user: data.user, token: data.token };
            } else {
                const error = await response.json();
                return { success: false, error: error.error || 'Erreur d\'inscription' };
            }
        } catch (e) {
            return { success: false, error: 'Erreur réseau' };
        }
    },

    async getCurrentUser(token: string): Promise<{ success: boolean; user?: User; error?: string }> {
        try {
            const response = await fetch(`${API_BASE}/me`, {
                headers: { 'Authorization': `Bearer ${token}` },
            });

            if (response.ok) {
                const user = await response.json();
                return { success: true, user };
            } else {
                return { success: false, error: 'Token invalide' };
            }
        } catch (e) {
            return { success: false, error: 'Erreur réseau' };
        }
    },
};
