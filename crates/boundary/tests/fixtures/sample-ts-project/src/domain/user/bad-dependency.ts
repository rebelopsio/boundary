import { PostgresUserRepository } from '../../infrastructure/postgres/user-repo';

// Intentional violation: domain depends on infrastructure
export function badFunction(): void {
    const repo = new PostgresUserRepository("bad");
}
