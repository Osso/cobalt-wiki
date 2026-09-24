-- Single-use links that let a user choose a password (member invites and
-- forgotten passwords). Only the SHA-256 of the token is stored.
CREATE TABLE password_token (
    token_id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(user_id),
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL CHECK (expires_at > created_at),
    emailed_at TIMESTAMP WITH TIME ZONE,
    used_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX password_token_user_idx ON password_token (user_id);
