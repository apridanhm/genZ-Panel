set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <backup_name>"
    echo "Example: $0 backup_20240115_120000"
    exit 1
fi

BACKUP_NAME=$1
BACKUP_DIR="./backups"

echo "Starting restore from $BACKUP_NAME..."

# Restore database
gunzip -c "$BACKUP_DIR/${BACKUP_NAME}_db.sql.gz" | docker compose exec -T postgres psql -U panel panel

# Restore apps
docker run --rm \
    -v hosting-panel_apps_data:/apps \
    -v "$(pwd)/$BACKUP_DIR:/backup" \
    alpine tar xzf "/backup/${BACKUP_NAME}_apps.tar.gz" -C /apps

echo "Restore completed!"