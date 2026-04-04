export const prerender = false;

export function load({ params }) {
  return { filename: decodeURIComponent(params.filename) };
}
