package com.example.domain.user;

import com.example.infrastructure.postgres.PostgresUserRepository;

// Intentional violation: domain depends on infrastructure
public class BadDependency {
    public void bad() {
        new PostgresUserRepository("bad");
    }
}
