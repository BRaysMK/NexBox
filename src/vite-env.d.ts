/// <reference types="vite/client" />

declare module "@/assets/*.png" {
  const value: string;
  export default value;
}

declare module "*.png" {
  const value: string;
  export default value;
}
