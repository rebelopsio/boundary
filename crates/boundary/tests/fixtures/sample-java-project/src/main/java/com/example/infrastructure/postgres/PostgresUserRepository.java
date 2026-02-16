package com.example.infrastructure.postgres;

import com.example.domain.user.User;
import com.example.domain.user.UserRepository;

public class PostgresUserRepository implements UserRepository {
    private final String connectionString;

    public PostgresUserRepository(String connectionString) {
        this.connectionString = connectionString;
    }

    public void save(User user) {
        System.out.println("saving user " + user.getId());
    }

    public User findById(String id) {
        return new User(id, "test", "test@example.com");
    }

    public void delete(String id) {
        System.out.println("deleting user " + id);
    }
}
