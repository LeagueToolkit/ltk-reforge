import { QueryClient } from "@tanstack/react-query";

/**
 * Every read of the store of runs. A run that is kept, a keep and a removal ask them all again:
 * the run page writes runs and the runs page reads them, and neither module imports the other.
 */
export const runsKey = ["runs"] as const;

/** The client every page shares. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,
      retry: 1,
    },
  },
});
