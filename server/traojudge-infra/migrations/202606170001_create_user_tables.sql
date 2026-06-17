CREATE TABLE users (
    id BIGINT SIGNED NOT NULL AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL,
    icon_object_key TEXT NULL,
    profile_traq_id TEXT NULL,
    x_id TEXT NULL,
    profile_github_id TEXT NULL,
    self_introduction TEXT NOT NULL DEFAULT '',
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY users_name_key (name)
);

CREATE TABLE user_global_authority_attributes (
    user_id BIGINT SIGNED NOT NULL,
    is_system_admin BOOLEAN NOT NULL DEFAULT FALSE,
    can_create_problem BOOLEAN NOT NULL DEFAULT FALSE,
    is_trap_user BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (user_id),
    CONSTRAINT user_global_authority_attributes_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
);
