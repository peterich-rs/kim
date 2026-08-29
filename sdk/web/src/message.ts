export class Content {
  constructor(
    public body: string,
    public type: number = 1,
    public extra: string = "",
  ) {}
}

export class Message {
  sender = "";
  receiver = "";
  group = "";
  type = 0;
  body = "";
  extra = "";
  arrivalTime = Date.now();
  contentLoaded = false;

  constructor(
    public messageId: bigint,
    public sendTime: bigint,
  ) {}
}

export class Response {
  constructor(
    public status: number,
    public dest: string = "",
    public payload: Uint8Array = new Uint8Array(),
  ) {}
}

export interface TalkResult {
  status: number;
  resp?: { messageId: bigint; sendTime: bigint };
  err?: Error;
}

export interface LoginBody {
  token: string;
  tags?: string[];
}
