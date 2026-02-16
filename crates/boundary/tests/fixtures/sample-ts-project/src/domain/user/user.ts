export interface UserRepository {
    save(user: User): Promise<void>;
    findById(id: string): Promise<User | null>;
    delete(id: string): Promise<void>;
}

export class User {
    constructor(
        public readonly id: string,
        public readonly name: string,
        public readonly email: string,
    ) {}
}
