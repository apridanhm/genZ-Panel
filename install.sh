#!/bin/bash
set -e

echo "🚀 Memulai instalasi dependensi GenZ Panel..."

# 1. Update package list
echo "📦 Updating package lists..."
apt-get update -y

# 2. Install dependensi dasar yang dibutuhkan panel
echo "📦 Menginstal dependensi dasar (git, unzip, curl, ca-certificates)..."
apt-get install -y git unzip curl ca-certificates

# 3. Pastikan Docker sudah terinstal (jika belum, instal)
if ! command -v docker &> /dev/null; then
    echo "🐳 Docker tidak ditemukan. Menginstal Docker..."
    curl -fsSL https://get.docker.com -o get-docker.sh
    sh get-docker.sh
    rm get-docker.sh
fi

# 4. Pastikan user saat ini ada di grup docker (opsional, tapi bagus untuk keamanan)
usermod -aG docker $USER 2>/dev/null || true

echo "✅ Semua dependensi host berhasil diinstal!"
echo "💡 Silakan jalankan 'docker compose up -d' atau mulai daemon panel Anda."
