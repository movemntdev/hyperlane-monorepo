import path from 'path';
import { types } from '@abacus-network/utils';
import {
  UpgradeBeaconController,
  XAppConnectionManager,
  ValidatorManager,
  Outbox,
  Inbox,
} from '@abacus-network/core';
import { CoreInstance } from './CoreInstance';
import { CoreContracts } from './CoreContracts';
import { CoreConfig } from './types';
import { ChainConfig, DeployEnvironment, RustConfig } from '../config';
import { CommonDeploy, DeployType } from '../common';

export class CoreDeploy extends CommonDeploy<CoreInstance, CoreConfig> {
  deployType = DeployType.CORE;

  deployInstance(
    domain: types.Domain,
    config: CoreConfig,
  ): Promise<CoreInstance> {
    return CoreInstance.deploy(domain, this.chains, config);
  }

  upgradeBeaconController(domain: types.Domain): UpgradeBeaconController {
    return this.instances[domain].upgradeBeaconController;
  }

  validatorManager(domain: types.Domain): ValidatorManager {
    return this.instances[domain].validatorManager;
  }

  outbox(domain: types.Domain): Outbox {
    return this.instances[domain].outbox;
  }

  inbox(local: types.Domain, remote: types.Domain): Inbox {
    return this.instances[local].inbox(remote);
  }

  xAppConnectionManager(domain: types.Domain): XAppConnectionManager {
    return this.instances[domain].xAppConnectionManager;
  }

  static readContracts(
    chains: Record<types.Domain, ChainConfig>,
    directory: string,
  ): CoreDeploy {
    return CommonDeploy.readContractsHelper(
      CoreDeploy,
      CoreInstance,
      CoreContracts.readJson,
      chains,
      directory,
    );
  }

  writeRustConfigs(environment: DeployEnvironment, directory: string) {
    for (const domain of this.domains) {
      const filepath = path.join(
        this.configDirectory(directory),
        'rust',
        `${this.name(domain)}_config.json`,
      );

      const outbox = {
        address: this.outbox(domain).address,
        domain: domain.toString(),
        name: this.name(domain),
        rpcStyle: 'ethereum',
        connection: {
          type: 'http',
          url: '',
        },
      };

      const rustConfig: RustConfig = {
        environment,
        signers: {
          [this.name(domain)]: { key: '', type: 'hexKey' },
        },
        replicas: {},
        home: outbox,
        tracing: {
          level: 'debug',
          fmt: 'json',
        },
        db: 'db_path',
      };

      for (const remote of this.remotes(domain)) {
        const inbox = {
          address: this.inbox(remote, domain).address,
          domain: remote.toString(),
          name: this.name(remote),
          rpcStyle: 'ethereum',
          connection: {
            type: 'http',
            url: '',
          },
        };

        rustConfig.signers[this.name(remote)] = { key: '', type: 'hexKey' };
        rustConfig.replicas[this.name(remote)] = inbox;
      }
      this.writeJson(filepath, rustConfig);
    }
  }
}