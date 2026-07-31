const http = require('http');
// Gunakan PORT dari environment variable (yang di-inject oleh Builder Daemon), atau fallback ke 5012
const port = process.env.PORT || 5012;

const server = http.createServer((req, res) => {
  res.statusCode = 200;
  res.setHeader('Content-Type', 'text/plain');
  res.end('🎉 SIUUUU! ZIP Deployment Berhasil & Server Sedang Berjalan! 🐐🔥\n');
});

server.listen(port, () => {
  console.log(`🚀 Server berjalan dan listening di port ${port}`);
});
