import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  server: {
    port: 3000,
    proxy: {
      // Proxy pour l'API d'authentification REST
      '/auth': {
        target: 'http://localhost:51051',
        changeOrigin: true,
        secure: false,
        configure: (proxy, _options) => {
          proxy.on('error', (err, _req, _res) => {
            console.log('proxy error', err);
          });
          proxy.on('proxyReq', (proxyReq, req, _res) => {
            console.log('Proxying:', req.method, req.url, '-> localhost:51051');
          });
        },
      },
      // Proxy pour gRPC-Web - redirige vers votre serveur Rust
      '/takeiteasygame.v1.SessionService': {
        target: 'http://localhost:50052',
        changeOrigin: true,
        headers: {
          'Content-Type': 'application/grpc-web+proto'
        }
      },
      '/takeiteasygame.v1.GameService': {
        target: 'http://localhost:50052',
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