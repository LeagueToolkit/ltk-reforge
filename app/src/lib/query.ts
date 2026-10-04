import { QueryClient } from "@tanstack/react-query";

/** The client every page shares. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,
      retry: 1,
    },
  },
});
