import Demo from './api'
import Project from '@hackbg/fadroma'

export default class DemoProject extends Project {

  Deployment = Demo

  // Override to customize the build command:
  //
  // build = async (...contracts: string[]) => { 
  //   await super.build(...contracts)
  // }

  // Override to customize the upload command:
  //
  // upload = async (...contracts: string[]) => {
  //   await super.upload(...contracts)
  // }

  // Override to customize the deploy command:
  //
  // deploy = async (...args: string[]) => {
  //   await super.deploy(...args)
  // }

  // Override to customize the status command:
  //
  // status = async (...args: string[]) => {
  //   await super.status()
  // }

  // Define custom commands using `this.command`:
  //
  // custom = this.command('custom', 'run a custom procedure', async () => {
  //   // ...
  // })

  listAuctions = this.command(
    'auction list',
    'list auctions',
    async (name: string, end: number) => {
      const deployment = (await this.deployment as Demo)
      console.log(await deployment.listAuctions())
    }
  )

  createAuction = this.command(
    'auction create',
    'create an auction',
    async (name: string, end: number = 0) => {
      const deployment = await this.deployment as Demo
      console.log(await deployment.createAuction(name, end))
    }
  )

}
