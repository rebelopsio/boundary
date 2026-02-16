import { User } from '../../domain/user/user';
import { UserRepository } from '../../domain/user/user';

export class PostgresUserRepository implements UserRepository {
    constructor(private readonly connectionString: string) {}

    async save(user: User): Promise<void> {
        console.log(`saving user ${user.id}`);
    }

    async findById(id: string): Promise<User | null> {
        return new User(id, 'test', 'test@example.com');
    }

    async delete(id: string): Promise<void> {
        console.log(`deleting user ${id}`);
    }
}
