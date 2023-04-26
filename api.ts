import { Address, Client, Deployment, Error } from '@fadroma/agent'

export default class Demo extends Deployment {

  auction = this.template<Auction>({ crate: "auction", client: Auction })

  auctionFactory = this.contract<AuctionFactory>({
    name: "AuctionFactory",
    crate: "factory",
    client: AuctionFactory,
    initMsg: async () => ({ auction: (await this.auction.uploaded).asInfo }),
  })

  createAuction = (name: string, end: number, admin: Address = this.agent?.address) =>
    this.auctionFactory.expect('factory must be deployed to create auction')
      .createAuction(name, end, admin)

  listAuctions = (start: number = 0, limit: number = 10) =>
    this.auctionFactory.expect('factory must be deployed to list auctions')
      .listAuctions(start, limit)

}

export class AuctionFactory extends Client {

  createAuction = (name: string, end: number, admin: Address = this.agent.address) => {
    if (!name) throw new Error('Pass auction name')
    return this.execute({ create_auction: { admin, name, end_block: end } })
  }

  listAuctions = (start: number = 0, limit: number = 10) =>
    this.query({ list_auctions: { pagination: { start, limit } } })

}

export class Auction extends Client {
  // Implement auction contract methods here...
}
