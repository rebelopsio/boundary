import { User } from '../domain/user/user';
import { UserRepository } from '../domain/user/user';

export class UserService {
    constructor(private readonly repo: UserRepository) {}

    async createUser(name: string, email: string): Promise<User> {
        const user = new User(crypto.randomUUID(), name, email);
        await this.repo.save(user);
        return user;
    }
}
