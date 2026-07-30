-- Add Git/ZIP support fields to applications table
ALTER TABLE applications 
ADD COLUMN IF NOT EXISTS source_type VARCHAR(20) DEFAULT 'git',
ADD COLUMN IF NOT EXISTS git_repo_url TEXT,
ADD COLUMN IF NOT EXISTS git_branch VARCHAR(255) DEFAULT 'main',
ADD COLUMN IF NOT EXISTS zip_file_path TEXT;

-- Rename fields for consistency (optional but recommended)
DO $$ 
BEGIN
    -- Rename language to runtime (kalau kolom language masih ada)
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'applications' AND column_name = 'language') THEN
        ALTER TABLE applications RENAME COLUMN language TO runtime;
    END IF;
    
    -- Rename port to exposed_port (kalau kolom port masih ada)
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'applications' AND column_name = 'port') THEN
        ALTER TABLE applications RENAME COLUMN port TO exposed_port;
    END IF;
END $$;

-- Add index for source_type
CREATE INDEX IF NOT EXISTS idx_applications_source_type ON applications(source_type);
