export type QueryView<T> =
  | { status: 'pending' }
  | { status: 'error'; message: string }
  | { status: 'success'; data: T; refreshing: boolean; warning?: string }
