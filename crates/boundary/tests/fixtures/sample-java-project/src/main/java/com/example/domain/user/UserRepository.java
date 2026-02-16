package com.example.domain.user;

public interface UserRepository {
    void save(User user);
    User findById(String id);
    void delete(String id);
}
