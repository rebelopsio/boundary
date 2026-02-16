package com.example.application;

import com.example.domain.user.User;
import com.example.domain.user.UserRepository;

public class UserService {
    private final UserRepository repo;

    public UserService(UserRepository repo) {
        this.repo = repo;
    }

    public User createUser(String name, String email) {
        User user = new User("generated-id", name, email);
        repo.save(user);
        return user;
    }
}
