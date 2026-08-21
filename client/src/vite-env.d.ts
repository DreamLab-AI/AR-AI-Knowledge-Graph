/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL?: string;
  readonly VITE_API_URL?: string;
  readonly VITE_WS_URL?: string;
  readonly VITE_NOSTR_RELAY?: string;
  readonly VITE_DEBUG?: string;
  readonly VITE_LOG_LEVEL?: string;
  readonly VITE_ENABLE_ANALYTICS?: string;
  readonly VITE_ENABLE_TELEMETRY?: string;
  readonly VITE_VIRCADIA_SERVER_URL?: string;
  readonly VITE_VIRCADIA_AUTH_TOKEN?: string;
  readonly VITE_VIRCADIA_AUTH_PROVIDER?: string;
  readonly VITE_VIRCADIA_ENABLED?: string;
  readonly VITE_VIRCADIA_ENABLE_MULTI_USER?: string;
  readonly VITE_VIRCADIA_ENABLE_SPATIAL_AUDIO?: string;
  readonly VITE_QUEST3_ENABLE_HAND_TRACKING?: string;
  readonly VITE_INSTANCED_RENDERING?: string;
  readonly VITE_JSS_URL?: string;
  readonly VITE_JSS_WS_URL?: string;
  readonly VITE_JSS_ONTOLOGY_PATH?: string;
  readonly VITE_DEBUG_REPLACE_CONSOLE?: string;
  readonly VITE_DEBUG_PRESET?: string;
  readonly VITE_DEBUG_CATEGORIES?: string;
  readonly VITE_DEV_MODE_AUTH?: string;
  readonly VITE_PUBLIC_DEMO?: string;
  readonly VITE_DEV_POWER_USER_PUBKEY?: string;
  readonly VITE_REMOTE_LOGGING_DISABLED?: string;
  readonly LOG_LEVEL?: string;
  readonly MODE: string;
  readonly DEV: boolean;
  readonly PROD: boolean;
  readonly SSR: boolean;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
