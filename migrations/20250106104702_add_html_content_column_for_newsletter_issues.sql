-- Add migration script here
ALTER TABLE newsletter_issues ADD COLUMN IF NOT EXISTS html_content TEXT NOT NULL;