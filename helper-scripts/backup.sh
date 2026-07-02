set -e

BACKUP_DIR="./backups"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="backup_${DATE}"

echo "Starting backup..."

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup database
docker compose exec -T postgres pg_dump -U panel panel > "$BACKUP_DIR/${BACKUP_NAME}_db.sql"

# Backup apps data
docker run --rm \
    -v hosting-panel_apps_data:/apps:ro \
    -v "$(pwd)/$BACKUP_DIR:/backup" \
    alpine tar czf "/backup/${BACKUP_NAME}_apps.tar.gz" -C /apps .

# Compress SQL
gzip "$BACKUP_DIR/${BACKUP_NAME}_db.sql"

echo "Backup completed: $BACKUP_DIR/$BACKUP_NAME"