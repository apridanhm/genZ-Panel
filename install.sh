#!/bin/bash

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "Script ini harus dijalankan sebagai root atau dengan sudo"
        exit 1
    fi
}

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
        VERSION=$VERSION_ID
        CODENAME=$VERSION_CODENAME
    else
        log_error "Tidak dapat mendeteksi OS"
        exit 1
    fi
    
    log_info "Detected OS: $OS $VERSION ($CODENAME)"
}

# Check if Docker is installed
check_docker() {
    if command -v docker &> /dev/null; then
        log_success "Docker sudah terinstall: $(docker --version)"
        return 0
    else
        return 1
    fi
}

# Install Docker based on OS
install_docker() {
    log_info "Menginstall Docker..."
    
    case "$OS" in
        ubuntu|debian)
            # Remove old versions
            apt-get remove -y docker docker-engine docker.io containerd runc 2>/dev/null || true
            
            # Install dependencies
            apt-get update
            apt-get install -y \
                apt-transport-https \
                ca-certificates \
                curl \
                gnupg \
                lsb-release
            
            # Add Docker's official GPG key
            install -m 0755 -d /etc/apt/keyrings
            curl -fsSL https://download.docker.com/linux/$OS/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
            chmod a+r /etc/apt/keyrings/docker.gpg
            
            # Add repository
            echo \
              "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/$OS \
              $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
            
            # Install Docker
            apt-get update
            apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
            ;;
            
        centos|rhel|almalinux|rocky)
            # Remove old versions
            yum remove -y docker docker-client docker-client-latest docker-common docker-latest docker-latest-logrotate docker-logrotate docker-engine 2>/dev/null || true
            
            # Install dependencies
            yum install -y yum-utils
            
            # Add repository
            yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
            
            # Install Docker
            yum install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
            ;;
            
        *)
            log_error "OS tidak didukung: $OS"
            log_info "OS yang didukung: Ubuntu, Debian, CentOS, RHEL, AlmaLinux, Rocky Linux"
            exit 1
            ;;
    esac
    
    # Start and enable Docker
    systemctl start docker
    systemctl enable docker
    
    log_success "Docker berhasil diinstall"
}

# Check Docker Compose
check_docker_compose() {
    if docker compose version &> /dev/null; then
        log_success "Docker Compose sudah terinstall: $(docker compose version --short)"
        return 0
    else
        return 1
    fi
}

# Generate configuration
generate_config() {
    if [ ! -f .env ]; then
        log_info "Generating configuration file..."
        
        # Generate random passwords
        DB_PASSWORD=$(openssl rand -base64 32)
        TRAEFIK_PASSWORD=$(openssl rand -base64 16)
        JWT_SECRET=$(openssl rand -base64 64)
        
        # Get server IP
        SERVER_IP=$(curl -s https://api.ipify.org 2>/dev/null || echo "YOUR_SERVER_IP")
        
        # Ask for domain
        echo ""
        read -p "Masukkan domain untuk panel (contoh: panel.example.com): " PANEL_DOMAIN
        
        if [ -z "$PANEL_DOMAIN" ]; then
            PANEL_DOMAIN="panel.local"
            log_warn "Domain tidak diisi, menggunakan default: $PANEL_DOMAIN"
        fi
        
        # Ask for email (for Let's Encrypt)
        read -p "Masukkan email untuk Let's Encrypt SSL: " LETSENCRYPT_EMAIL
        
        if [ -z "$LETSENCRYPT_EMAIL" ]; then
            LETSENCRYPT_EMAIL="admin@$PANEL_DOMAIN"
            log_warn "Email tidak diisi, menggunakan default: $LETSENCRYPT_EMAIL"
        fi
        
        # Generate .env file
        cat > .env <<EOF
# Panel Configuration
PANEL_DOMAIN=$PANEL_DOMAIN
SERVER_IP=$SERVER_IP
LETSENCRYPT_EMAIL=$LETSENCRYPT_EMAIL

# Database
POSTGRES_PASSWORD=$DB_PASSWORD
POSTGRES_USER=panel
POSTGRES_DB=panel

# Security
JWT_SECRET=$JWT_SECRET
TRAEFIK_DASHBOARD_PASSWORD=$TRAEFIK_PASSWORD

# Timezone
TZ=Asia/Jakarta

# Logging
LOG_LEVEL=info
EOF
        
        log_success "Configuration file generated: .env"
        log_warn "PENTING: Backup file .env Anda! Ini berisi password database dan secret keys."
    else
        log_warn "File .env sudah ada, skipping configuration generation"
    fi
}

# Pull Docker images
pull_images() {
    log_info "Pulling Docker images (ini mungkin memakan waktu beberapa menit)..."
    docker compose pull
    log_success "Docker images berhasil di-pull"
}

# Start services
start_services() {
    log_info "Starting services..."
    docker compose up -d
    
    log_info "Waiting for services to be ready..."
    sleep 10
    
    # Check if all services are running
    if docker compose ps | grep -q "Exit"; then
        log_error "Beberapa service gagal start. Cek log dengan: docker compose logs"
        exit 1
    fi
    
    log_success "All services started successfully"
}

# Print access information
print_info() {
    source .env
    
    echo ""
    echo "=========================================="
    echo -e "${GREEN}Panel Hosting berhasil diinstall!${NC}"
    echo "=========================================="
    echo ""
    echo "Access URL: https://$PANEL_DOMAIN"
    echo ""
    echo "Admin Credentials:"
    echo "  Email: admin@$PANEL_DOMAIN"
    echo "  Password: (check logs with: docker compose logs core-api | grep 'Admin password')"
    echo ""
    echo "Traefik Dashboard: https://$PANEL_DOMAIN:8080"
    echo "  Username: admin"
    echo "  Password: $TRAEFIK_DASHBOARD_PASSWORD"
    echo ""
    echo "Useful Commands:"
    echo "  View logs:        docker compose logs -f"
    echo "  Stop panel:       docker compose down"
    echo "  Start panel:      docker compose up -d"
    echo "  Restart panel:    docker compose restart"
    echo "  Update panel:     ./update.sh"
    echo "  Backup:           ./helper-scripts/backup.sh"
    echo ""
    echo "Documentation: https://docs.yourpanel.com"
    echo "=========================================="
    echo ""
}

# Main installation flow
main() {
    echo ""
    echo "=========================================="
    echo "  Panel Hosting Installer"
    echo "=========================================="
    echo ""
    
    check_root
    detect_os
    
    # Install Docker if not present
    if ! check_docker; then
        install_docker
    fi
    
    # Check Docker Compose
    if ! check_docker_compose; then
        log_error "Docker Compose tidak ditemukan. Pastikan Docker terinstall dengan benar."
        exit 1
    fi
    
    # Generate configuration
    generate_config
    
    # Pull images
    pull_images
    
    # Start services
    start_services
    
    # Print access info
    print_info
}

# Run main function
main "$@"