import { Client, Deployment } from '@fadroma/agent'

export default class Demo extends Deployment {
  supplier = this.contract({
    name: "supplier",
    crate: "supplier",
    client: Supplier,
    initMsg: async () => ({})
  })
  distributor = this.contract({
    name: "distributor",
    crate: "distributor",
    client: Distributor,
    initMsg: async () => ({})
  })

  // Add contract with::
  //   contract = this.contract({...})
  //
  // Add contract from fadroma.json with:
  //   contract = this.template('name').instance({...})

}

export class Supplier extends Client {
  // Implement methods calling the contract here:
  //
  // async myTx (arg1, arg2) {
  //   return await this.execute({ my_tx: { arg1, arg2 }})
  // }
  // async myQuery (arg1, arg2) {
  //   return await this.query({ my_query: { arg1, arg2 } })
  // }
  //
  // or like this:
  //
  // myTx = (arg1, arg2) => this.execute({my_tx:{arg1, arg2}})
  // myQuery = (arg1, arg2) => this.query({my_query:{arg1, arg2}})
  //
}


export class Distributor extends Client {
  // Implement methods calling the contract here:
  //
  // async myTx (arg1, arg2) {
  //   return await this.execute({ my_tx: { arg1, arg2 }})
  // }
  // async myQuery (arg1, arg2) {
  //   return await this.query({ my_query: { arg1, arg2 } })
  // }
  //
  // or like this:
  //
  // myTx = (arg1, arg2) => this.execute({my_tx:{arg1, arg2}})
  // myQuery = (arg1, arg2) => this.query({my_query:{arg1, arg2}})
  //
}
