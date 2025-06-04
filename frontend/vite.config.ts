import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  server: {
    port: 3000,
    proxy: {
      // Proxy pour gRPC-Web - redirige vers votre serveur Rust
      '/takeiteasygame.v1.SessionService': {
        target: 'http://localhost:50051',
        changeOrigin: true,
        headers: {
          'Content-Type': 'application/grpc-web+proto'
        }
      },
      '/takeiteasygame.v1.GameService': {
        target: 'http://localhost:50051',
        changeOrigin: true,
        headers: {
          'Content-Type': 'application/grpc-web+proto'
        }
      }
    }
  },
  define: {
    // Variables d'environnement pour le développement
    __GRPC_WEB_ENDPOINT__: JSON.stringify('http://localhost:50051'),
  },
  optimizeDeps: {
    // Inclure les dépendances gRPC-Web dans l'optimisation
    include: [
      '@protobuf-ts/runtime',
      '@protobuf-ts/runtime-rpc',
      '@protobuf-ts/grpcweb-transport'
    ]
  }
});