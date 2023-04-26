import { Client, Deployment, Pagination } from '@fadroma/agent'

export default class Demo extends Deployment {

  auctionFactory = this.contract({
    name:    "AuctionFactory",
    crate:   "factory",
    client:  AuctionFactory,
    initMsg: async () => ({
      auction: (await this.auction.uploaded).asInfo
    })
  })

  auction = this.template({
    crate:  "auction",
    client: Auction,
  })

  createAuction (name: string, end: number, admin: Address = this.agent?.address) {
    return this.auctionFactory.expect().createAuction(name, end, admin)
  }

  get auctions () {
    const factory = this.auctionFactory.expect('factory must be deployed to list auctions')
    return new Promise(async (resolve) => {
      let auctions = []
      let page = []
      while (page = await factory.listAuctions({ start: auctions.length, page: 10 })) {
        this.log.log('Fetched auctions:', page)
        auctions = [...auctions, ...page]
      }
      resolve(this.auction.instances(auctions))
    })
  }

}

export class AuctionFactory extends Client {

  createAuction = (name: string, end: number, admin: Address = this.agent.address) =>
    this.execute({ create_auction: { admin, name, end_block: end } })

  listAuctions = (pagination?: Pagination) =>
    this.query({ list_auctions: { pagination } })

}

export class Auction extends Client {}
