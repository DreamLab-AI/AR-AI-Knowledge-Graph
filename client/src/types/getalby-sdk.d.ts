declare module '@getalby/sdk' {
  export class WebLNProvider {
    constructor();
    getPublicKey(): Promise<string>;
    signEvent(event: any): Promise<any>;
  }
}