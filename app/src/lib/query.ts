import { QueryClient } from "@tanstack/react-query";

/**
 * Every read of the store of runs. A run that is kept, a keep and a removal ask them all again:
 * the run page writes runs and the runs page reads them, and neither module imports the other.
 */
export const runsKey = ["runs"] as const;

/**
 * The mutations of the backend's run job, which runs one at a time: the run page's run and a
 * batch. Each page asks whether either is under way by this prefix, and neither module imports
 * the other's key.
 */
export const jobKey = ["job"] as const;

/** The client every page shares. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,
      retry: 1,
    },
  },
});
