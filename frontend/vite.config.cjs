const { defineConfig } = require('vite')
const solidPlugin = require('vite-plugin-solid')

module.exports = defineConfig({
  plugins: [solidPlugin()],
  server: {
    port: 3000
  }
})
