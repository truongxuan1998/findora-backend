import { Redis } from '@upstash/redis'

// Upstash Redis Client initialized from environment variables
// Automatically loads UPSTASH_REDIS_REST_URL and UPSTASH_REDIS_REST_TOKEN
export const redis = Redis.fromEnv()

export default redis
