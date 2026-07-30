CREATE TABLE IF NOT EXISTS applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    user_id UUID NOT NULL, -- Denormalized for easier querying
    
    name VARCHAR(255) NOT NULL,
    runtime VARCHAR(50) NOT NULL, -- 'php', 'node', 'go', 'python', 'rust'
    runtime_version VARCHAR(50),  -- e.g., '18', '8.3', '1.21'
    
    source_type VARCHAR(20) NOT NULL DEFAULT 'git', -- 'git' or 'zip'
    git_repo_url TEXT,
    git_branch VARCHAR(255) DEFAULT 'main',
    zip_file_path TEXT, -- Path to uploaded zip if source_type = 'zip'
    
    build_command TEXT, -- e.g., 'npm run build' or 'go build'
    start_command TEXT NOT NULL, -- e.g., 'npm start' or 'php artisan serve'
    exposed_port INTEGER NOT NULL DEFAULT 3000,
    
    cpu_limit FLOAT DEFAULT 0.5, -- e.g., 0.5 = 50% of 1 core
    ram_limit_mb INTEGER DEFAULT 512,
    
    status VARCHAR(50) DEFAULT 'pending', -- 'pending', 'building', 'running', 'stopped', 'failed'
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_applications_domain_id ON applications(domain_id);
CREATE INDEX idx_applications_user_id ON applications(user_id);
CREATE INDEX idx_applications_status ON applications(status);
