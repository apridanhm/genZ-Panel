-- Add SSL fields to domains table
ALTER TABLE domains 
ADD COLUMN IF NOT EXISTS ssl_provider VARCHAR(20) DEFAULT 'letsencrypt',
ADD COLUMN IF NOT EXISTS ssl_cert_path VARCHAR(512),
ADD COLUMN IF NOT EXISTS ssl_key_path VARCHAR(512),
ADD COLUMN IF NOT EXISTS ssl_expires_at TIMESTAMP WITH TIME ZONE;

-- Add index for auto-renewal queries
CREATE INDEX IF NOT EXISTS idx_domains_ssl_expires ON domains(ssl_expires_at) 
WHERE ssl_provider = 'letsencrypt';
