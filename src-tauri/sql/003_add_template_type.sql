-- Migration 003: Add template_type column to inspection_templates
-- This column was missing from the original schema but is used by the Rust models

ALTER TABLE inspection_templates ADD COLUMN template_type TEXT DEFAULT 'ssh';

-- Update existing records to have 'ssh' as the default template_type
UPDATE inspection_templates SET template_type = 'ssh' WHERE template_type IS NULL;
